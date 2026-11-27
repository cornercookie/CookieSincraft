//! GPU upscaler backed by `wgpu`.
//!
//! Provides `GpuUpscaler`, a compute-shader implementation of the `Upscaler`
//! trait that performs offscreen upscaling on the GPU. Supported algorithms:
//! FSR 1.0 (EASU + RCAS), NIS, Anime4K (CNN x2 M), and a plain bilinear path.
//!
//! On wgpu initialization failure or shader execution failure, an
//! `UpscaleError` is returned so the caller can fall back to the CPU.
//! This module never panics.

use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use bytemuck::{cast_slice, Pod, Zeroable};
use image::GenericImageView;
use wgpu::util::DeviceExt;

use crate::upscaler::{Algorithm, Backend, UpscaleError, UpscaleJob, Upscaler};

// FSR 1.0 RCAS sharpness: named constant, NOT an upstream-specified default.
// 非上游指定預設：ffx_fsr1.h FsrRcasCon(stops) 註解未給出具體預設 stops 值。
const RCAS_DEFAULT_STOPS: f32 = 0.33;

// Anime4K CNN runs only when both axes scale > 1.2 (upstream //!WHEN).
const ANIME4K_WHEN_SCALE: f32 = 1.2;

/// GPU-based upscaler wrapping a `wgpu::Device` and `wgpu::Queue`.
pub struct GpuUpscaler {
    device: wgpu::Device,
    queue: wgpu::Queue,
    error_flag: Arc<Mutex<bool>>,
    last_error: Arc<Mutex<String>>,
    sampler: wgpu::Sampler,
    /// Shared bind group layout (binding 0 texture, 1 sampler, 2 storage tex,
    /// 3 params uniform). Used by bilinear / FSR RCAS / NIS single-texture
    /// passes. Storage texture is Rgba8Unorm.
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for the FSR EASU pass: identical to `bind_group_layout`
    /// except the storage texture is `Rgba16Float` to match `fsr_easu.wgsl`'s
    /// `dest: texture_storage_2d<rgba16float, write>`. Without this the EASU
    /// pipeline cannot be created and `GpuUpscaler::new()` fails, dragging
    /// Bilinear down to the CPU fallback with it.
    fsr_easu_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout with two coefficient storage buffers (binding 4, 5)
    /// for NIS coef_scale / coef_usm.
    nis_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout with two source textures (binding 0, 1) for Anime4K
    /// depth-to-space only. conv2d_last uses `anime_last_bind_group_layout`.
    anime_dual_bind_group_layout: wgpu::BindGroupLayout,
    /// conv2d_last_tf: seven feature maps (0–6), sampler (7), storage (8), uniform (9).
    /// Storage texture (binding 8) is rgba16float to preserve signed unclamped
    /// feature-map values from the conv passes.
    anime_last_bind_group_layout: wgpu::BindGroupLayout,
    /// Anime4K conv passes (0–6): single texture pass with rgba16float storage
    /// texture (binding 2) so feature maps keep signed unclamped float values.
    anime_conv_bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// Pipeline layout for the FSR EASU pass (uses `fsr_easu_bind_group_layout`).
    fsr_easu_pipeline_layout: wgpu::PipelineLayout,
    nis_pipeline_layout: wgpu::PipelineLayout,
    anime_dual_pipeline_layout: wgpu::PipelineLayout,
    anime_last_pipeline_layout: wgpu::PipelineLayout,
    /// Pipeline layout for Anime4K conv passes (uses `anime_conv_bind_group_layout`).
    anime_conv_pipeline_layout: wgpu::PipelineLayout,

    /// Single, explicitly-named GPU path: bilinear sampling + mild sharpening.
    bilinear_pipeline: wgpu::ComputePipeline,
    /// FSR 1.0 pass 1: EASU (writes rgba16float intermediate).
    fsr_easu_pipeline: wgpu::ComputePipeline,
    /// FSR 1.0 pass 2: RCAS (reads EASU output).
    fsr_rcas_pipeline: wgpu::ComputePipeline,
    /// NIS NVScaler (reads coef_scale / coef_usm storage buffers).
    nis_pipeline: wgpu::ComputePipeline,
    /// Anime4K CNN x2 M: 9 compute passes (one pipeline each).
    anime_passes: [wgpu::ComputePipeline; 9],
}

/// Pass names for the 9 Anime4K hooks (in pipeline order).
const ANIME_PASS_NAMES: [&str; 9] = [
    "conv2d_tf",
    "conv2d_1_tf",
    "conv2d_2_tf",
    "conv2d_3_tf",
    "conv2d_4_tf",
    "conv2d_5_tf",
    "conv2d_6_tf",
    "conv2d_last_tf",
    "depth_to_space",
];

/// Params uniform uploaded to shaders: 5 f32 + 3 f32 padding (16-byte align).
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
struct ParamsUniform {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

/// GPU budget limits enforced before every real GPU resource allocation.
/// Max dimension: 4096. Max buffer: 64 MiB per resource.
const GPU_MAX_DIM: u32 = 4096;
const GPU_MAX_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB

/// Bytes per pixel for a given texture format.
fn gpu_format_bytes_per_pixel(format: wgpu::TextureFormat) -> u64 {
    match format {
        wgpu::TextureFormat::Rgba16Float => 8,
        wgpu::TextureFormat::Rgba8Unorm => 4,
        _ => 4,
    }
}

/// Validate a GPU texture allocation against dimension and byte budget limits
/// *before* the texture is created. Returns an `Err` (which callers turn into
/// a CPU fallback reason) if the resource would exceed limits, never faking
/// a successful GPU allocation.
fn check_gpu_texture_budget(
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> Result<(), UpscaleError> {
    if width > GPU_MAX_DIM || height > GPU_MAX_DIM {
        return Err(UpscaleError::BackendUnavailable(format!(
            "GPU texture '{label}' {width}x{height} exceeds max dimension {GPU_MAX_DIM}"
        )));
    }
    let bytes = (width as u64) * (height as u64) * gpu_format_bytes_per_pixel(format);
    if bytes > GPU_MAX_BYTES {
        return Err(UpscaleError::BackendUnavailable(format!(
            "GPU texture '{label}' {width}x{height} {:?} ({bytes} bytes) exceeds 64 MiB budget",
            format
        )));
    }
    Ok(())
}

/// Validate a row-aligned staging buffer allocation against dimension and byte
/// budget limits *before* the buffer is created.
fn check_gpu_staging_budget(width: u32, height: u32, label: &str) -> Result<(), UpscaleError> {
    if width > GPU_MAX_DIM || height > GPU_MAX_DIM {
        return Err(UpscaleError::BackendUnavailable(format!(
            "Staging buffer '{label}' {width}x{height} exceeds max dimension {GPU_MAX_DIM}"
        )));
    }
    let dst_row_bytes = width as usize * 4;
    let dst_padded_row = (dst_row_bytes + 255) / 256 * 256;
    let staging_size = dst_padded_row * height as usize;
    if staging_size as u64 > GPU_MAX_BYTES {
        return Err(UpscaleError::BackendUnavailable(format!(
            "Staging buffer '{label}' {width}x{height} row-aligned ({staging_size} bytes) exceeds 64 MiB budget"
        )));
    }
    Ok(())
}

/// FSR EASU constants: 4 x vec4 = 16 f32 (64 bytes), 16-byte aligned.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
struct FsrEasuConstants {
    con0: [f32; 4],
    con1: [f32; 4],
    con2: [f32; 4],
    con3: [f32; 4],
}

/// FSR RCAS constant: 1 x vec4 = 4 f32.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
struct FsrRcasConstants {
    con: [f32; 4],
    _pad: [f32; 3],
}

impl GpuUpscaler {
    /// Create a new GPU upscaler using the DX12 backend.
    pub fn new() -> Result<Self, UpscaleError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());

        let adapter = instance
            .enumerate_adapters(wgpu::Backends::DX12)
            .into_iter()
            .next()
            .ok_or_else(|| UpscaleError::BackendUnavailable("no DX12 adapter found".to_string()))?;

        let required_features = wgpu::Features::empty();
        let desc = wgpu::DeviceDescriptor {
            label: Some("cookie-sincraft gpu upscaler device"),
            required_features,
            required_limits: wgpu::Limits::default(),
        };

        let (device, queue) = Self::block_on(&instance, adapter.request_device(&desc, None))
            .map_err(|e| UpscaleError::BackendUnavailable(format!("request_device failed: {e:?}")))?;

        // Error tracking to avoid panics on validation errors.
        let error_flag = Arc::new(Mutex::new(false));
        let last_error = Arc::new(Mutex::new(String::new()));
        let error_flag_clone = error_flag.clone();
        let last_error_clone = last_error.clone();
        device.on_uncaptured_error(Box::new(move |err| {
            #[cfg(debug_assertions)]
            tracing::warn!("wgpu uncaptured error");
            *error_flag_clone.lock().unwrap() = true;
            let mut slot = last_error_clone.lock().unwrap();
            if !slot.is_empty() {
                slot.push_str("\n---\n");
            }
            slot.push_str(&err.to_string());
        }));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gpu upscaler sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // ---- Bind group layout: single texture pass (binding 0..3) ----
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu upscaler bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ---- Bind group layout: FSR EASU (storage texture = Rgba16Float) ----
        // Matches `fsr_easu.wgsl`'s `dest: texture_storage_2d<rgba16float, write>`.
        let fsr_easu_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu fsr easu bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba16Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // ---- Bind group layout: NIS with two coefficient buffers (binding 4,5) ----
        let nis_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu nis bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // ---- Bind group layout: Anime4K dual-source (binding 0,1 textures) ----
        let anime_dual_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu anime dual bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let mut anime_last_entries: Vec<wgpu::BindGroupLayoutEntry> =
            (0..7).map(tex_entry).collect();
        anime_last_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        anime_last_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 8,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: wgpu::TextureFormat::Rgba16Float,
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        });
        anime_last_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 9,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        let anime_last_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu anime last bind group layout"),
                entries: &anime_last_entries,
            });

        // ---- Bind group layout: Anime4K conv passes (storage = rgba16float) ----
        // Feature maps must keep signed unclamped float values, so the storage
        // texture is rgba16float rather than rgba8unorm.
        let anime_conv_entries: Vec<wgpu::BindGroupLayoutEntry> = vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let anime_conv_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpu anime conv bind group layout"),
                entries: &anime_conv_entries,
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu upscaler pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
        let fsr_easu_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu fsr easu pipeline layout"),
                bind_group_layouts: &[&fsr_easu_bind_group_layout],
                push_constant_ranges: &[],
            });
        let nis_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu nis pipeline layout"),
                bind_group_layouts: &[&nis_bind_group_layout],
                push_constant_ranges: &[],
            });
        let anime_dual_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu anime dual pipeline layout"),
                bind_group_layouts: &[&anime_dual_bind_group_layout],
                push_constant_ranges: &[],
            });
        let anime_last_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu anime last pipeline layout"),
                bind_group_layouts: &[&anime_last_bind_group_layout],
                push_constant_ranges: &[],
            });
        let anime_conv_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu anime conv pipeline layout"),
                bind_group_layouts: &[&anime_conv_bind_group_layout],
                push_constant_ranges: &[],
            });

        // ---- Shader modules ----
        let bilinear_shader = Self::create_shader_module_safe(
            &device,
            "bilinear shader",
            include_str!("../shaders/bilinear.wgsl"),
        )?;
        let fsr_easu_shader = Self::create_shader_module_safe(
            &device,
            "fsr easu shader",
            include_str!("../shaders/fsr_easu.wgsl"),
        )?;
        let fsr_rcas_shader = Self::create_shader_module_safe(
            &device,
            "fsr rcas shader",
            include_str!("../shaders/fsr_rcas.wgsl"),
        )?;
        let nis_shader = Self::create_shader_module_safe(
            &device,
            "nis scaler shader",
            include_str!("../shaders/nis_scaler.wgsl"),
        )?;

        // Anime4K pass shaders (one module per hook).
        let anime_shaders: [wgpu::ShaderModule; 9] = [
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_1_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_1_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_2_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_2_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_3_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_3_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_4_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_4_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_5_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_5_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_6_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_6_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime conv2d_last_tf shader",
                include_str!("../shaders/anime4k_cnn_passes/conv2d_last_tf.wgsl"),
            )?,
            Self::create_shader_module_safe(
                &device,
                "anime depth_to_space shader",
                include_str!("../shaders/anime4k_cnn_passes/MAIN.wgsl"),
            )?,
        ];

        let bilinear_pipeline = Self::create_compute_pipeline_safe(
            &device,
            &pipeline_layout,
            "bilinear pipeline",
            &bilinear_shader,
        )?;
        let fsr_easu_pipeline = Self::create_compute_pipeline_safe(
            &device,
            &fsr_easu_pipeline_layout,
            "fsr easu pipeline",
            &fsr_easu_shader,
        )?;
        let fsr_rcas_pipeline = Self::create_compute_pipeline_safe(
            &device,
            &pipeline_layout,
            "fsr rcas pipeline",
            &fsr_rcas_shader,
        )?;
        let nis_pipeline = Self::create_compute_pipeline_safe(
            &device,
            &nis_pipeline_layout,
            "nis scaler pipeline",
            &nis_shader,
        )?;

        let mut anime_passes_vec: Vec<wgpu::ComputePipeline> = Vec::with_capacity(9);
        for (i, module) in anime_shaders.iter().enumerate() {
            // Only depth-to-space (pass 8) uses the dual-source layout.
            // conv2d_last (pass 7) binds seven feature maps; see anime_last layout.
            let layout = if i == 8 {
                &anime_dual_pipeline_layout
            } else if i == 7 {
                &anime_last_pipeline_layout
            } else {
                // Conv passes 0–6 use the rgba16float-storage layout so
                // feature maps keep signed unclamped float values.
                &anime_conv_pipeline_layout
            };
            anime_passes_vec.push(Self::create_compute_pipeline_safe(
                &device,
                layout,
                &format!("anime {} pipeline", ANIME_PASS_NAMES[i]),
                module,
            )?);
        }
        let anime_passes: [wgpu::ComputePipeline; 9] = anime_passes_vec
            .try_into()
            .ok()
            .ok_or_else(|| UpscaleError::Unknown("anime pipeline count mismatch".to_string()))?;

        let _ = device.poll(wgpu::Maintain::Wait);
        if *error_flag.lock().unwrap() {
            let detail = last_error.lock().unwrap().clone();
            return Err(UpscaleError::BackendUnavailable(format!(
                "GPU pipeline validation failed during init: {detail}"
            )));
        }

        Ok(Self {
            device,
            queue,
            error_flag,
            last_error,
            sampler,
            bind_group_layout,
            fsr_easu_bind_group_layout,
            nis_bind_group_layout,
            anime_dual_bind_group_layout,
            anime_last_bind_group_layout,
            anime_conv_bind_group_layout,
            pipeline_layout,
            fsr_easu_pipeline_layout,
            nis_pipeline_layout,
            anime_dual_pipeline_layout,
            anime_last_pipeline_layout,
            anime_conv_pipeline_layout,
            bilinear_pipeline,
            fsr_easu_pipeline,
            fsr_rcas_pipeline,
            nis_pipeline,
            anime_passes,
        })
    }

    /// Safely create a shader module, catching panics from validation errors.
    fn create_shader_module_safe(
        device: &wgpu::Device,
        label: &str,
        source: &str,
    ) -> Result<wgpu::ShaderModule, UpscaleError> {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(source)),
            })
        }));

        match result {
            Ok(module) => Ok(module),
            Err(_) => Err(UpscaleError::BackendUnavailable(format!(
                "shader module creation panicked (likely WGSL validation error): {label}"
            ))),
        }
    }

    /// Safely create a compute pipeline, catching panics from validation errors.
    fn create_compute_pipeline_safe(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        label: &str,
        module: &wgpu::ShaderModule,
    ) -> Result<wgpu::ComputePipeline, UpscaleError> {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                module,
                entry_point: "main",
            })
        }));

        match result {
            Ok(pipeline) => Ok(pipeline),
            Err(_) => Err(UpscaleError::BackendUnavailable(format!(
                "compute pipeline creation panicked (likely WGSL validation error): {label}"
            ))),
        }
    }

    /// Block on a future by repeatedly polling `instance` until it completes.
    fn block_on<F: std::future::Future>(instance: &wgpu::Instance, future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let waker = Self::noop_waker();
        let mut cx = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
            instance.poll_all(true);
        }
    }

    fn noop_waker() -> Waker {
        unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &NOOP_VTABLE)) }
    }
}

static NOOP_VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_noop, noop, wake_by_ref_noop, drop_noop);

unsafe fn clone_noop(_: *const ()) -> RawWaker {
    RawWaker::new(ptr::null(), &NOOP_VTABLE)
}
unsafe fn noop(_: *const ()) {}
unsafe fn wake_by_ref_noop(_: *const ()) {}
unsafe fn drop_noop(_: *const ()) {}

impl Upscaler for GpuUpscaler {
    fn upscale_with_control(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        control: crate::upscaler::UpscaleControl,
    ) -> Result<image::DynamicImage, UpscaleError> {
        // Precheck cancelled.
        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled(
                "GPU upscale cancelled".to_string(),
            ));
        }

        // Check error flag from previous operations.
        if *self.error_flag.lock().unwrap() {
            return Err(UpscaleError::BackendUnavailable(format!(
                "previous GPU operation left device in error state: {}",
                self.last_error.lock().unwrap()
            )));
        }

        let result = match job.algorithm {
            Algorithm::Bilinear => self.execute_single_pass(
                job, image, &self.bilinear_pipeline, 0.0,
                Some(&control),
            ),
            Algorithm::Fsr1_0 => self.execute_fsr1_0(job, image, &control),
            Algorithm::Nis => self.execute_nis(job, image, Some(&control)),
            Algorithm::Anime4k => self.execute_anime4k(job, image),
            other => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "GPU upscaler does not support algorithm {other:?}"
                )));
            }
        };

        // Preserve typed result before error flag check.
        if matches!(&result, Err(UpscaleError::Cancelled(_))) { return result; }

        // Check cancellation before error flag so user cancel takes precedence.
        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled(
                "GPU upscale cancelled".to_string(),
            ));
        }

        // Check error flag after execution.
        if *self.error_flag.lock().unwrap() {
            return Err(UpscaleError::BackendUnavailable(format!(
                "GPU execution produced validation errors: {}",
                self.last_error.lock().unwrap()
            )));
        }

        result
    }

    fn upscale(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
    ) -> Result<image::DynamicImage, UpscaleError> {
        self.upscale_with_control(job, image, crate::upscaler::UpscaleControl::none())
    }

    fn supports_backend(&self, backend: Backend) -> bool {
        matches!(backend, Backend::Gpu)
    }

    fn fallback_available(&self) -> bool {
        true
    }
}

// ============================================================================
// Single-texture execute path (bilinear / FSR / NIS).
// ============================================================================

/// Read a texture back to an RGBA8 image.
fn readback_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<image::DynamicImage, UpscaleError> {
    check_gpu_staging_budget(width, height, "gpu upscaler readback staging")?;
    let dst_row_bytes = width as usize * 4;
    let dst_padded_row = (dst_row_bytes + 255) / 256 * 256;
    let staging_size = dst_padded_row * height as usize;
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu upscaler readback staging"),
        size: staging_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gpu upscaler readback encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(dst_padded_row as u32),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    loop {
        let result = device.poll(wgpu::Maintain::Wait);
        if let Ok(Ok(_)) = rx.try_recv() {
            break;
        }
        if result.is_queue_empty() {
            let _ = device.poll(wgpu::Maintain::Wait);
            break;
        }
    }

    let mapped = slice.get_mapped_range();
    let mapped_bytes: &[u8] = bytemuck::cast_slice(&*mapped);
    let mut dst_rgba = Vec::with_capacity(dst_row_bytes * height as usize);
    for row in 0..height as usize {
        let start = row * dst_padded_row;
        dst_rgba.extend_from_slice(&mapped_bytes[start..start + dst_row_bytes]);
    }
    drop(mapped);
    staging_buffer.unmap();

    let dst_buffer = image::ImageBuffer::from_raw(width, height, dst_rgba)
        .ok_or_else(|| UpscaleError::Unknown("readback image has invalid dimensions".to_string()))?;
    Ok(image::DynamicImage::ImageRgba8(dst_buffer))
}

impl GpuUpscaler {
    /// Create source texture + padded upload for an RGBA8 image.
    fn create_source_texture(&self, image: &image::DynamicImage) -> Result<(wgpu::Texture, u32, u32), UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let src_rgba = image.to_rgba8();
        let src_bytes = src_rgba.as_raw();
        let row_bytes = src_width as usize * 4;
        let padded_row = (row_bytes + 255) / 256 * 256;
        let mut padded_src = Vec::with_capacity(padded_row * src_height as usize);
        for row in 0..src_height as usize {
            let start = row * row_bytes;
            padded_src.extend_from_slice(&src_bytes[start..start + row_bytes]);
            if padded_row > row_bytes {
                padded_src.extend(std::iter::repeat(0u8).take(padded_row - row_bytes));
            }
        }
        check_gpu_texture_budget(src_width, src_height, wgpu::TextureFormat::Rgba8Unorm, "gpu upscaler source")?;
        let source_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu upscaler source"),
            size: wgpu::Extent3d {
                width: src_width,
                height: src_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &source_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &padded_src,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_row as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: src_width,
                height: src_height,
                depth_or_array_layers: 1,
            },
        );
        Ok((source_texture, src_width, src_height))
    }

    /// Single-pass execute: bind source -> pipeline -> dest storage texture -> readback.
    fn execute_single_pass(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        pipeline: &wgpu::ComputePipeline,
        sharpness: f32,
        control: Option<&crate::upscaler::UpscaleControl>,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let dst_width = job.target_width;
        let dst_height = job.target_height;

        let (source_texture, _, _) = self.create_source_texture(image)?;
        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        check_gpu_texture_budget(dst_width, dst_height, wgpu::TextureFormat::Rgba8Unorm, "gpu upscaler dest")?;
        let dest_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu upscaler dest"),
            size: wgpu::Extent3d {
                width: dst_width,
                height: dst_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let dest_view = dest_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let params = ParamsUniform {
            src_width: src_width as f32,
            src_height: src_height as f32,
            dst_width: dst_width as f32,
            dst_height: dst_height as f32,
            sharpness,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gpu upscaler params"),
            contents: cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu upscaler bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&dest_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut last_report: Option<std::time::Instant> = None;
        self.dispatch_and_readback(
            pipeline,
            &bind_group,
            dst_width,
            dst_height,
            &dest_texture,
            control,
            &mut last_report,
        )
    }

    fn dispatch_and_readback(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: &wgpu::BindGroup,
        dst_width: u32,
        dst_height: u32,
        dest_texture: &wgpu::Texture,
        control: Option<&crate::upscaler::UpscaleControl>,
        last_report: &mut Option<std::time::Instant>,
    ) -> Result<image::DynamicImage, UpscaleError> {
        // Check cancellation before submit so a user cancel is immediately
        // effective without submitting work.
        if let Some(control) = control {
            if control.is_cancelled() {
                return Err(UpscaleError::Cancelled(
                    "GPU upscale cancelled".to_string(),
                ));
            }
        }

        self.device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu upscaler command encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu upscaler compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(
                (dst_width + 15) / 16,
                (dst_height + 15) / 16,
                1,
            );
        }
        if let Some(control) = control {
            if control.is_cancelled() {
                drop(encoder);
                let _ = futures::executor::block_on(self.device.pop_error_scope());
                let _ = futures::executor::block_on(self.device.pop_error_scope());
                return Err(UpscaleError::Cancelled(
                    "GPU upscale cancelled".to_string(),
                ));
            }
        }
        let submission_index = self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        // Pop both scopes LIFO and check for GPU errors after submission.
        let oom_err = futures::executor::block_on(self.device.pop_error_scope());
        let validation_err = futures::executor::block_on(self.device.pop_error_scope());
        if let Some(err) = oom_err.or(validation_err) {
            *self.error_flag.lock().unwrap() = true;
            return Err(UpscaleError::BackendUnavailable(format!(
                "GPU error scope: {err:?}"
            )));
        }

        // Check cancellation before reporting so callback returns typed Cancelled.
        if let Some(control) = control {
            if control.is_cancelled() {
                return Err(UpscaleError::Cancelled(
                    "GPU upscale cancelled".to_string(),
                ));
            }
        }

        // Release lock before callbacks by reading error flag into bool
        // so no successful completion report for a GPU error state.
        let error_flag = *self.error_flag.lock().unwrap();
        if error_flag {
            return Err(UpscaleError::BackendUnavailable(
                "GPU execution produced validation errors".to_string(),
            ));
        }

        // Report progress after GPU completion.
        let groups = (((dst_width + 15) / 16) * ((dst_height + 15) / 16)) as usize;
        if let Some(control) = control {
            control.report_progress(groups, groups, last_report);
        }

        // Recheck immediately before texture readback so callback cancellation
        // returns typed Cancelled.
        if control.map_or(false, |c| c.is_cancelled()) {
            return Err(UpscaleError::Cancelled(
                "GPU upscale cancelled".to_string(),
            ));
        }

        let result = readback_texture(&self.device, &self.queue, &dest_texture, dst_width, dst_height);

        // Honor cancellation after readback like FSR and outer trait.
        if control.map_or(false, |c| c.is_cancelled()) {
            return Err(UpscaleError::Cancelled(
                "GPU upscale cancelled".to_string(),
            ));
        }

        result
    }

    // ========================================================================
    // FSR 1.0
    // ========================================================================

fn execute_fsr1_0(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        control: &crate::upscaler::UpscaleControl,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let dst_width = job.target_width;
        let dst_height = job.target_height;

        // EASU supports any upscale ratio (1x-4x area). No 1x-2x restriction.
        if dst_width < src_width || dst_height < src_height {
            return Err(UpscaleError::UnsupportedAlgorithm(
                "FSR 1.0 does not support downscaling".to_string(),
            ));
        }

        // Precheck cancelled.
        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled(
                "FSR 1.0 upscale cancelled".to_string(),
            ));
        }

        let mut last_report = None;

        // Check GPU texture budget before FSR intermediate allocation.
        check_gpu_texture_budget(
            dst_width,
            dst_height,
            wgpu::TextureFormat::Rgba16Float,
            "FSR EASU intermediate",
        )?;

        // ---- Pass 1: EASU (writes rgba16float intermediate) ----
        // Create an rgba16float intermediate texture sized to dst.
        let intermediate = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fsr easu intermediate"),
            size: wgpu::Extent3d {
                width: dst_width,
                height: dst_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let intermediate_view = intermediate.create_view(&wgpu::TextureViewDescriptor::default());

        // Source texture (RGBA8 -> sampled as float).
        let (source_texture, _, _) = self.create_source_texture(image)?;
        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // FsrEasuCon (ffx_fsr1.h:156-202), computed on CPU.
        // con0 = [vpW/dstW, vpH/dstH, 0.5*vpW/dstW-0.5, 0.5*vpH/dstH-0.5]
        // con1 = [1/srcW, 1/srcH,  1/srcW,           -1/srcH          ]
        // con2 = [-1/srcW, 2/srcH,  1/srcW,           2/srcH           ]
        // con3 = [0, 4/srcH, 0, 0]
        let vp_w = src_width as f32;
        let vp_h = src_height as f32;
        let in_w = src_width as f32;
        let in_h = src_height as f32;
        let out_w = dst_width as f32;
        let out_h = dst_height as f32;
        let easu_consts = FsrEasuConstants {
            con0: [
                vp_w / out_w,
                vp_h / out_h,
                0.5 * vp_w / out_w - 0.5,
                0.5 * vp_h / out_h - 0.5,
            ],
            con1: [1.0 / in_w, 1.0 / in_h, 1.0 / in_w, -1.0 / in_h],
            con2: [-1.0 / in_w, 2.0 / in_h, 1.0 / in_w, 2.0 / in_h],
            con3: [0.0, 4.0 / in_h, 0.0, 0.0],
        };
        let easu_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fsr easu constants"),
            contents: cast_slice(&[easu_consts]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let easu_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fsr easu bind group"),
            layout: &self.fsr_easu_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&intermediate_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: easu_buffer.as_entire_binding(),
                },
            ],
        });

        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fsr easu command encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fsr easu compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.fsr_easu_pipeline);
            pass.set_bind_group(0, &easu_bind_group, &[]);
            pass.dispatch_workgroups(
                (dst_width + 15) / 16,
                (dst_height + 15) / 16,
                1,
            );
        }

        if control.is_cancelled() {
            drop(encoder);
            let _ = futures::executor::block_on(self.device.pop_error_scope());
            let _ = futures::executor::block_on(self.device.pop_error_scope());
            return Err(UpscaleError::Cancelled(
                "FSR 1.0 upscale cancelled".to_string(),
            ));
        }

        let submission_index = self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        // Pop both scopes LIFO and check for GPU errors after EASU submission.
        let oom_err = futures::executor::block_on(self.device.pop_error_scope());
        let validation_err = futures::executor::block_on(self.device.pop_error_scope());
        if let Some(err) = oom_err.or(validation_err) {
            *self.error_flag.lock().unwrap() = true;
            return Err(UpscaleError::BackendUnavailable(format!(
                "FSR EASU GPU error scope: {err:?}"
            )));
        }

        // Check cancellation before reporting.
        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled(
                "FSR 1.0 upscale cancelled".to_string(),
            ));
        }

        // Report progress after GPU completion.
        let groups = (((dst_width + 15) / 16) * ((dst_height + 15) / 16)) as usize;
        control.report_progress(groups, 2 * groups, &mut last_report);

        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled("FSR 1.0 upscale cancelled".to_string()));
        }

        // ---- RCAS pass ----
        // Push error scopes before RCAS encoder and resources creation
        // (moved from after EASU to here, before RCAS encoder creation).
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);

        // ---- Pass 2: RCAS (reads intermediate, writes rgba8unorm dest) ----
        // RCAS sharpness: stops -> sharpness = exp2(-stops).
        // Default stops: RCAS_DEFAULT_STOPS (非上游指定預設).
        let stops = RCAS_DEFAULT_STOPS;
        let sharpness = (-stops).exp2();
        let rcas_consts = FsrRcasConstants {
            con: [sharpness, sharpness, 0.0, 0.0],
            _pad: [0.0, 0.0, 0.0],
        };
        let rcas_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fsr rcas constants"),
            contents: cast_slice(&[rcas_consts]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let dest_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fsr rcas dest"),
            size: wgpu::Extent3d {
                width: dst_width,
                height: dst_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let dest_view = dest_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Intermediate readback texture must be sampled as float; bind it as
        // source texture binding 0.
        let intermediate_sample_view =
            intermediate.create_view(&wgpu::TextureViewDescriptor::default());

        let rcas_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fsr rcas bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&intermediate_sample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&dest_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: rcas_buffer.as_entire_binding(),
                },
            ],
        });

        // RCAS compute pass.
        let mut rcas_encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fsr rcas command encoder"),
            });
        {
            let mut pass = rcas_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fsr rcas compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.fsr_rcas_pipeline);
            pass.set_bind_group(0, &rcas_bind_group, &[]);
            pass.dispatch_workgroups(
                (dst_width + 15) / 16,
                (dst_height + 15) / 16,
                1,
            );
        }

        // After RCAS encoding: if cancelled drop encoder and pop both scopes,
        // else submit+wait, pop/check both scopes, check cancel before readback and after reading;
        // return actual readback unchanged.
        // Check if cancelled after RCAS encoding.
        if control.is_cancelled() {
            // drop encoder and pop both scopes LIFO then return Cancelled.
            drop(rcas_encoder);
            let _ = futures::executor::block_on(self.device.pop_error_scope());
            let _ = futures::executor::block_on(self.device.pop_error_scope());
            return Err(UpscaleError::Cancelled(
                "FSR 1.0 upscale cancelled".to_string(),
            ));
        }

        // Submit RCAS encoder and wait for GPU completion.
        let rcas_submission_index = self.queue.submit(Some(rcas_encoder.finish()));
        self.device.poll(wgpu::Maintain::WaitForSubmissionIndex(rcas_submission_index));

        // Pop/check both scopes and check GPU errors after RCAS GPU work.
        let oom_err = futures::executor::block_on(self.device.pop_error_scope());
        let validation_err = futures::executor::block_on(self.device.pop_error_scope());
        if let Some(err) = oom_err.or(validation_err) {
            *self.error_flag.lock().unwrap() = true;
            return Err(UpscaleError::BackendUnavailable(format!(
                "FSR RCAS GPU error scope: {err:?}"
            )));
        }

        // Check cancel before readback.
        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled(
                "FSR 1.0 upscale cancelled".to_string(),
            ));
        }

        // Readback.
        let result = readback_texture(&self.device, &self.queue, &dest_texture, dst_width, dst_height);

        // Report progress after GPU completion if result is ok and not cancelled.
        let device_ok = !*self.error_flag.lock().unwrap();
        if result.is_ok() && !control.is_cancelled() && device_ok {
            let groups = (((dst_width + 15) / 16) * ((dst_height + 15) / 16)) as usize;
            control.report_progress(2 * groups, 2 * groups, &mut last_report);
            // Recheck cancellation after callback.
            if control.is_cancelled() {
                return Err(UpscaleError::Cancelled(
                    "FSR 1.0 upscale cancelled".to_string(),
                ));
            }
        }

        // Check cancel after reading.
        if control.is_cancelled() {
            return Err(UpscaleError::Cancelled(
                "FSR 1.0 upscale cancelled".to_string(),
            ));
        }

        result
    }

    // ========================================================================
    // NIS
    // ========================================================================

    fn execute_nis(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        control: Option<&crate::upscaler::UpscaleControl>,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let dst_width = job.target_width;
        let dst_height = job.target_height;

        // NIS supports 1x-2x per axis (kScale 0.5-1, i.e. scale 1.0-2.0).
        let scale_x = src_width as f32 / dst_width as f32;
        let scale_y = src_height as f32 / dst_height as f32;
        if scale_x < 0.5 || scale_x > 1.0 || scale_y < 0.5 || scale_y > 1.0 {
            // Outside NIS range: signal Err so commands.rs falls back to CPU Lanczos3.
            return Err(UpscaleError::UnsupportedAlgorithm(format!(
                "NIS supports 1x-2x per axis, got scale_x={scale_x:.3} scale_y={scale_y:.3}"
            )));
        }

        // Coefficients from NIS_Config.h coef_scale / coef_usm (64 phases x 8 taps).
        // Upload full 64x8 table; shader indexes [phase][0..5] (kFilterSize=6).
        let coef_scale = *nis_coef_scale();
        let coef_usm = *nis_coef_usm();
        let coef_scale_bytes: Vec<u8> = coef_scale
            .iter()
            .flat_map(|row| row.iter().copied().flat_map(|v| v.to_le_bytes()))
            .collect();
        let coef_usm_bytes: Vec<u8> = coef_usm
            .iter()
            .flat_map(|row| row.iter().copied().flat_map(|v| v.to_le_bytes()))
            .collect();
        let coef_scale_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nis coef_scale"),
            contents: &coef_scale_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let coef_usm_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nis coef_usm"),
            contents: &coef_usm_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let (source_texture, _, _) = self.create_source_texture(image)?;
        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let dest_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nis dest"),
            size: wgpu::Extent3d {
                width: dst_width,
                height: dst_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let dest_view = dest_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Sharpness: sharpness - 0.5 mapping; GPU path uses 0.5 as midpoint.
        // Default 0.5 => no sharpening offset.
        let sharpness = 0.5;
        let params = ParamsUniform {
            src_width: src_width as f32,
            src_height: src_height as f32,
            dst_width: dst_width as f32,
            dst_height: dst_height as f32,
            sharpness,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nis params"),
            contents: cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nis bind group"),
            layout: &self.nis_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&dest_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: coef_scale_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: coef_usm_buffer.as_entire_binding(),
                },
            ],
        });

        let mut last_report: Option<std::time::Instant> = None;
        self.dispatch_and_readback(
            &self.nis_pipeline,
            &bind_group,
            dst_width,
            dst_height,
            &dest_texture,
            control,
            &mut last_report,
        )
    }

    // ========================================================================
    // Anime4K CNN x2 M
    // ========================================================================

    fn execute_anime4k(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let dst_width = job.target_width;
        let dst_height = job.target_height;

        let scale_x = dst_width as f32 / src_width as f32;
        let scale_y = dst_height as f32 / src_height as f32;

        // Anime4K CNN x2 M runs only when both axes scale > 1.2 (upstream //!WHEN).
        // Otherwise fall back to existing bilinear pipeline; do NOT mark used_fallback
        // as Anime4K failure.
        if scale_x <= ANIME4K_WHEN_SCALE || scale_y <= ANIME4K_WHEN_SCALE {
            return self.execute_single_pass(job, image, &self.bilinear_pipeline, 0.0, None);
        }

        let mut current_w = src_width;
        let mut current_h = src_height;
        let mut current: Option<image::DynamicImage> = None;

        // Run full 2x CNN stages while either dimension hasn't reached the target
        // target. Preflight every stage allocation (checked
        // doubling, per-texture 64 MiB incl. RGBA16Float feature map, device
        // texture dimension, RGBA8 readback stride); on overbudget return
        // BackendUnavailable via the budget helpers — never a silent break.
        while current_w < dst_width || current_h < dst_height {
            let next_w = current_w.checked_mul(2).ok_or_else(|| {
                UpscaleError::BackendUnavailable(format!(
                    "anime4k checked doubling overflow {current_w}x{current_h}"
                ))
            })?;
            let next_h = current_h.checked_mul(2).ok_or_else(|| {
                UpscaleError::BackendUnavailable(format!(
                    "anime4k checked doubling overflow {current_w}x{current_h}"
                ))
            })?;
            // Validate next dimensions against max texture dimension limit.
            let max_dim = self.device.limits().max_texture_dimension_2d;
            if next_w > max_dim || next_h > max_dim {
                return Err(UpscaleError::BackendUnavailable(format!(
                    "Anime4K intermediate {next_w}x{next_h} exceeds device limit {max_dim}"
                )));
            }
            check_gpu_texture_budget(
                current_w,
                current_h,
                wgpu::TextureFormat::Rgba16Float,
                "anime4k cnn feature map",
            )?;
            check_gpu_texture_budget(
                next_w,
                next_h,
                wgpu::TextureFormat::Rgba8Unorm,
                "anime4k cnn dest",
            )?;
            check_gpu_staging_budget(next_w, next_h, "anime4k cnn readback")?;
            let src = current.as_ref().unwrap_or(image);
            current = Some(self.run_anime_cnn(src, current_w, current_h, next_w, next_h)?);
            current_w = next_w;
            current_h = next_h;
        }

        // Clone the original only when no CNN stage ran; otherwise current is
        // the CNN-upscaled image — never clone just to launch the first pass.
        let current = current.unwrap_or_else(|| image.clone());

        // Exact dimension guard: CNN result IS the final result (exact 2x chain).
        if current_w == dst_width && current_h == dst_height {
            return Ok(current);
        }

        // Whole-frame GPU Bilinear from the CNN-upscaled `current` (not the
        // original) to the exact target size, keeping the whole route on GPU.
        self.execute_single_pass(job, &current, &self.bilinear_pipeline, 0.0, None)
    }

    /// Run the 9-pass Anime4K CNN pipeline from `src` (size src_w x src_h) to
    /// `dst` (size dst_w x dst_h), where dst must equal 2x src (single CNN 2x).
    ///
    /// Upstream-correct layout: all conv passes (0–6) are resolution-preserving
    /// at MAIN resolution (src_w x src_h). conv2d_last (pass 7) fuses the seven
    /// feature maps at MAIN resolution. depth_to_space (pass 8) reads the
    /// conv2d_last output plus the original MAIN image and writes 2x (dst_w x
    /// dst_h). Feature maps are stored as rgba16float so signed unclamped
    /// values (including negative relu_neg contributions) are preserved.
    fn run_anime_cnn(
        &self,
        src: &image::DynamicImage,
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
    ) -> Result<image::DynamicImage, UpscaleError> {
        // Create RGBA8 source texture (float sampled).
        let (src_tex, _, _) = self.create_source_texture(src)?;
        let src_view = src_tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Intermediate chain: pass output becomes next pass input. All conv
        // passes are resolution-preserving at MAIN resolution (src_w x src_h);
        // depth_to_space writes 2x (dst_w x dst_h).
        //
        // For conv passes, input resolution == output resolution (the CNN is
        // resolution-preserving; the 2x comes from depth_to_space at the end).
        // So we render all conv passes at MAIN resolution reading from the
        // previous pass's output (which is MAIN-sized).
        //
        // First pass reads the original src (src_w x src_h). Subsequent conv
        // passes read the previous MAIN-sized output.

        let mut anime_pairs: Vec<(wgpu::Texture, wgpu::TextureView)> = Vec::new();

        // Helper: run a single-texture conv pass (pipeline index i), writing to
        // a fresh src_w x src_h rgba16float texture.
        let run_single = |pipeline_idx: usize,
                              input_view: &wgpu::TextureView,
                              w: u32,
                              h: u32| -> Result<(wgpu::Texture, wgpu::TextureView), UpscaleError> {
            check_gpu_texture_budget(w, h, wgpu::TextureFormat::Rgba16Float, "anime conv pass output")?;
            let out = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("anime conv pass output"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let out_view = out.create_view(&wgpu::TextureViewDescriptor::default());

            let params = ParamsUniform {
                src_width: w as f32,
                src_height: h as f32,
                dst_width: w as f32,
                dst_height: h as f32,
                sharpness: 0.0,
                _pad0: 0.0,
                _pad1: 0.0,
                _pad2: 0.0,
            };
            let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("anime params"),
                contents: cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("anime conv bind group"),
                layout: &self.anime_conv_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&out_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("anime pass command encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("anime pass compute pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.anime_passes[pipeline_idx]);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups((w + 15) / 16, (h + 15) / 16, 1);
            }
            self.queue.submit(Some(encoder.finish()));
            let validation_err = futures::executor::block_on(self.device.pop_error_scope());
            let oom_err = futures::executor::block_on(self.device.pop_error_scope());
            if let Some(err) = validation_err.or(oom_err) {
                *self.error_flag.lock().unwrap() = true;
                return Err(UpscaleError::BackendUnavailable(format!(
                    "Anime4K pass {} GPU error scope: {err:?}",
                    ANIME_PASS_NAMES[pipeline_idx]
                )));
            }
            Ok((out, out_view))
        };

        // Passes 0-6: conv2d_tf .. conv2d_6_tf (resolution-preserving at MAIN
        // resolution). Pass 0 reads the source; others read the previous output.
        let mut cur_view = &src_view;
        for i in 0..7 {
            let (out, out_view) = run_single(i, cur_view, src_w, src_h)?;
            anime_pairs.push((out, out_view));
            cur_view = &anime_pairs.last().unwrap().1;
        }

        // Pass 7: conv2d_last_tf reads the seven feature maps (bindings 0–6).
        if anime_pairs.len() != 7 {
            return Err(UpscaleError::Unknown(
                "Anime4K conv2d_last expected 7 feature maps".to_string(),
            ));
        }
        check_gpu_texture_budget(src_w, src_h, wgpu::TextureFormat::Rgba16Float, "anime conv2d_last output")?;
        let out7 = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("anime conv2d_last output"),
            size: wgpu::Extent3d {
                width: src_w,
                height: src_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let out7_view = out7.create_view(&wgpu::TextureViewDescriptor::default());
        let last_params = ParamsUniform {
            src_width: src_w as f32,
            src_height: src_h as f32,
            dst_width: src_w as f32,
            dst_height: src_h as f32,
            sharpness: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let last_params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("anime conv2d_last params"),
            contents: cast_slice(&[last_params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let last_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("anime conv2d_last bind group"),
            layout: &self.anime_last_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[0].1),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[1].1),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[2].1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[3].1),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[4].1),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[5].1),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&anime_pairs[6].1),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&out7_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: last_params_buffer.as_entire_binding(),
                },
            ],
        });
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("anime conv2d_last command encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("anime conv2d_last compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.anime_passes[7]);
            pass.set_bind_group(0, &last_bind, &[]);
            pass.dispatch_workgroups((src_w + 15) / 16, (src_h + 15) / 16, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        let validation_err = futures::executor::block_on(self.device.pop_error_scope());
        let oom_err = futures::executor::block_on(self.device.pop_error_scope());
        if let Some(err) = validation_err.or(oom_err) {
            *self.error_flag.lock().unwrap() = true;
            return Err(UpscaleError::BackendUnavailable(format!(
                "Anime4K pass conv2d_last_tf GPU error scope: {err:?}"
            )));
        }
        drop(last_bind);
        let last_view = &out7_view;

        // Pass 8: depth_to_space — dual source: source0 = conv2d_last output
        // (MAIN-sized, rgba16float), source1 = original MAIN image (src_w x src_h,
        // rgba8unorm). Writes 2x = dst_w x dst_h as rgba8unorm.
        let params = ParamsUniform {
            src_width: src_w as f32,
            src_height: src_h as f32,
            dst_width: dst_w as f32,
            dst_height: dst_h as f32,
            sharpness: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("anime depth2space params"),
            contents: cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        check_gpu_texture_budget(dst_w, dst_h, wgpu::TextureFormat::Rgba8Unorm, "anime depth2space dest")?;
        let d2s_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("anime depth2space dest"),
            size: wgpu::Extent3d {
                width: dst_w,
                height: dst_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let d2s_view = d2s_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let d2s_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("anime depth2space bind group"),
            layout: &self.anime_dual_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&last_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&d2s_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("anime depth2space command encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("anime depth2space compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.anime_passes[8]);
            pass.set_bind_group(0, &d2s_bind_group, &[]);
            pass.dispatch_workgroups((dst_w + 15) / 16, (dst_h + 15) / 16, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        let validation_err = futures::executor::block_on(self.device.pop_error_scope());
        let oom_err = futures::executor::block_on(self.device.pop_error_scope());
        if let Some(err) = validation_err.or(oom_err) {
            *self.error_flag.lock().unwrap() = true;
            return Err(UpscaleError::BackendUnavailable(format!(
                "Anime4K depth_to_space GPU error scope: {err:?}"
            )));
        }

        readback_texture(&self.device, &self.queue, &d2s_tex, dst_w, dst_h)
    }
}

/// NIS coef_scale table (64 phases x 8 taps) from NIS_Config.h:274
/// (commit 35e13ba). Shader indexes [phase][0..5] (kFilterSize=6).
const NIS_COEF_SCALE: [[f32; 8]; 64] = [
    [0f32, 0f32, 1f32, 0f32, 0f32, 0f32, 0f32, 0f32],
    [0.0029f32, -0.0127f32, 1f32, 0.0132f32, -0.0034f32, 0f32, 0f32, 0f32],
    [0.0063f32, -0.0249f32, 0.9985f32, 0.0269f32, -0.0068f32, 0f32, 0f32, 0f32],
    [0.0088f32, -0.0361f32, 0.9956f32, 0.0415f32, -0.0103f32, 0.0005f32, 0f32, 0f32],
    [0.0117f32, -0.0474f32, 0.9932f32, 0.0562f32, -0.0142f32, 0.0005f32, 0f32, 0f32],
    [0.0142f32, -0.0576f32, 0.9897f32, 0.0713f32, -0.0181f32, 0.0005f32, 0f32, 0f32],
    [0.0166f32, -0.0674f32, 0.9844f32, 0.0874f32, -0.022f32, 0.001f32, 0f32, 0f32],
    [0.0186f32, -0.0762f32, 0.9785f32, 0.104f32, -0.0264f32, 0.0015f32, 0f32, 0f32],
    [0.0205f32, -0.085f32, 0.9727f32, 0.1206f32, -0.0308f32, 0.002f32, 0f32, 0f32],
    [0.0225f32, -0.0928f32, 0.9648f32, 0.1382f32, -0.0352f32, 0.0024f32, 0f32, 0f32],
    [0.0239f32, -0.1006f32, 0.9575f32, 0.1558f32, -0.0396f32, 0.0029f32, 0f32, 0f32],
    [0.0254f32, -0.1074f32, 0.9487f32, 0.1738f32, -0.0439f32, 0.0034f32, 0f32, 0f32],
    [0.0264f32, -0.1138f32, 0.939f32, 0.1929f32, -0.0488f32, 0.0044f32, 0f32, 0f32],
    [0.0278f32, -0.1191f32, 0.9282f32, 0.2119f32, -0.0537f32, 0.0049f32, 0f32, 0f32],
    [0.0288f32, -0.1245f32, 0.917f32, 0.231f32, -0.0581f32, 0.0059f32, 0f32, 0f32],
    [0.0293f32, -0.1294f32, 0.9058f32, 0.251f32, -0.063f32, 0.0063f32, 0f32, 0f32],
    [0.0303f32, -0.1333f32, 0.8926f32, 0.271f32, -0.0679f32, 0.0073f32, 0f32, 0f32],
    [0.0308f32, -0.1367f32, 0.8789f32, 0.2915f32, -0.0728f32, 0.0083f32, 0f32, 0f32],
    [0.0308f32, -0.1401f32, 0.8657f32, 0.312f32, -0.0776f32, 0.0093f32, 0f32, 0f32],
    [0.0313f32, -0.1426f32, 0.8506f32, 0.333f32, -0.0825f32, 0.0103f32, 0f32, 0f32],
    [0.0313f32, -0.1445f32, 0.8354f32, 0.354f32, -0.0874f32, 0.0112f32, 0f32, 0f32],
    [0.0313f32, -0.146f32, 0.8193f32, 0.3755f32, -0.0923f32, 0.0122f32, 0f32, 0f32],
    [0.0313f32, -0.147f32, 0.8022f32, 0.3965f32, -0.0967f32, 0.0137f32, 0f32, 0f32],
    [0.0308f32, -0.1479f32, 0.7856f32, 0.4185f32, -0.1016f32, 0.0146f32, 0f32, 0f32],
    [0.0303f32, -0.1479f32, 0.7681f32, 0.4399f32, -0.106f32, 0.0156f32, 0f32, 0f32],
    [0.0298f32, -0.1479f32, 0.7505f32, 0.4614f32, -0.1104f32, 0.0166f32, 0f32, 0f32],
    [0.0293f32, -0.147f32, 0.7314f32, 0.4829f32, -0.1147f32, 0.0181f32, 0f32, 0f32],
    [0.0288f32, -0.146f32, 0.7119f32, 0.5049f32, -0.1187f32, 0.019f32, 0f32, 0f32],
    [0.0278f32, -0.1445f32, 0.6929f32, 0.5264f32, -0.1226f32, 0.02f32, 0f32, 0f32],
    [0.0273f32, -0.1431f32, 0.6724f32, 0.5479f32, -0.126f32, 0.0215f32, 0f32, 0f32],
    [0.0264f32, -0.1411f32, 0.6528f32, 0.5693f32, -0.1299f32, 0.0225f32, 0f32, 0f32],
    [0.0254f32, -0.1387f32, 0.6323f32, 0.5903f32, -0.1328f32, 0.0234f32, 0f32, 0f32],
    [0.0244f32, -0.1357f32, 0.6113f32, 0.6113f32, -0.1357f32, 0.0244f32, 0f32, 0f32],
    [0.0234f32, -0.1328f32, 0.5903f32, 0.6323f32, -0.1387f32, 0.0254f32, 0f32, 0f32],
    [0.0225f32, -0.1299f32, 0.5693f32, 0.6528f32, -0.1411f32, 0.0264f32, 0f32, 0f32],
    [0.0215f32, -0.126f32, 0.5479f32, 0.6724f32, -0.1431f32, 0.0273f32, 0f32, 0f32],
    [0.02f32, -0.1226f32, 0.5264f32, 0.6929f32, -0.1445f32, 0.0278f32, 0f32, 0f32],
    [0.019f32, -0.1187f32, 0.5049f32, 0.7119f32, -0.146f32, 0.0288f32, 0f32, 0f32],
    [0.0181f32, -0.1147f32, 0.4829f32, 0.7314f32, -0.147f32, 0.0293f32, 0f32, 0f32],
    [0.0166f32, -0.1104f32, 0.4614f32, 0.7505f32, -0.1479f32, 0.0298f32, 0f32, 0f32],
    [0.0156f32, -0.106f32, 0.4399f32, 0.7681f32, -0.1479f32, 0.0303f32, 0f32, 0f32],
    [0.0146f32, -0.1016f32, 0.4185f32, 0.7856f32, -0.1479f32, 0.0308f32, 0f32, 0f32],
    [0.0137f32, -0.0967f32, 0.3965f32, 0.8022f32, -0.147f32, 0.0313f32, 0f32, 0f32],
    [0.0122f32, -0.0923f32, 0.3755f32, 0.8193f32, -0.146f32, 0.0313f32, 0f32, 0f32],
    [0.0112f32, -0.0874f32, 0.354f32, 0.8354f32, -0.1445f32, 0.0313f32, 0f32, 0f32],
    [0.0103f32, -0.0825f32, 0.333f32, 0.8506f32, -0.1426f32, 0.0313f32, 0f32, 0f32],
    [0.0093f32, -0.0776f32, 0.312f32, 0.8657f32, -0.1401f32, 0.0308f32, 0f32, 0f32],
    [0.0083f32, -0.0728f32, 0.2915f32, 0.8789f32, -0.1367f32, 0.0308f32, 0f32, 0f32],
    [0.0073f32, -0.0679f32, 0.271f32, 0.8926f32, -0.1333f32, 0.0303f32, 0f32, 0f32],
    [0.0063f32, -0.063f32, 0.251f32, 0.9058f32, -0.1294f32, 0.0293f32, 0f32, 0f32],
    [0.0059f32, -0.0581f32, 0.231f32, 0.917f32, -0.1245f32, 0.0288f32, 0f32, 0f32],
    [0.0049f32, -0.0537f32, 0.2119f32, 0.9282f32, -0.1191f32, 0.0278f32, 0f32, 0f32],
    [0.0044f32, -0.0488f32, 0.1929f32, 0.939f32, -0.1138f32, 0.0264f32, 0f32, 0f32],
    [0.0034f32, -0.0439f32, 0.1738f32, 0.9487f32, -0.1074f32, 0.0254f32, 0f32, 0f32],
    [0.0029f32, -0.0396f32, 0.1558f32, 0.9575f32, -0.1006f32, 0.0239f32, 0f32, 0f32],
    [0.0024f32, -0.0352f32, 0.1382f32, 0.9648f32, -0.0928f32, 0.0225f32, 0f32, 0f32],
    [0.002f32, -0.0308f32, 0.1206f32, 0.9727f32, -0.085f32, 0.0205f32, 0f32, 0f32],
    [0.0015f32, -0.0264f32, 0.104f32, 0.9785f32, -0.0762f32, 0.0186f32, 0f32, 0f32],
    [0.001f32, -0.022f32, 0.0874f32, 0.9844f32, -0.0674f32, 0.0166f32, 0f32, 0f32],
    [0.0005f32, -0.0181f32, 0.0713f32, 0.9897f32, -0.0576f32, 0.0142f32, 0f32, 0f32],
    [0.0005f32, -0.0142f32, 0.0562f32, 0.9932f32, -0.0474f32, 0.0117f32, 0f32, 0f32],
    [0.0005f32, -0.0103f32, 0.0415f32, 0.9956f32, -0.0361f32, 0.0088f32, 0f32, 0f32],
    [0f32, -0.0068f32, 0.0269f32, 0.9985f32, -0.0249f32, 0.0063f32, 0f32, 0f32],
    [0f32, -0.0034f32, 0.0132f32, 1f32, -0.0127f32, 0.0029f32, 0f32, 0f32],
];

/// NIS coef_usm table (64 phases x 8 taps) from NIS_Config.h:341
const NIS_COEF_USM: [[f32; 8]; 64] = [
    [0f32, -0.6001f32, 1.2002f32, -0.6001f32, 0f32, 0f32, 0f32, 0f32],
    [0.0029f32, -0.6084f32, 1.1987f32, -0.5903f32, -0.0029f32, 0f32, 0f32, 0f32],
    [0.0049f32, -0.6147f32, 1.1958f32, -0.5791f32, -0.0068f32, 0.0005f32, 0f32, 0f32],
    [0.0073f32, -0.6196f32, 1.189f32, -0.5659f32, -0.0103f32, 0f32, 0f32, 0f32],
    [0.0093f32, -0.6235f32, 1.1802f32, -0.5513f32, -0.0151f32, 0f32, 0f32, 0f32],
    [0.0112f32, -0.6265f32, 1.1699f32, -0.5352f32, -0.0195f32, 0.0005f32, 0f32, 0f32],
    [0.0122f32, -0.627f32, 1.1582f32, -0.5181f32, -0.0259f32, 0.0005f32, 0f32, 0f32],
    [0.0142f32, -0.6284f32, 1.1455f32, -0.5005f32, -0.0317f32, 0.0005f32, 0f32, 0f32],
    [0.0156f32, -0.6265f32, 1.1274f32, -0.479f32, -0.0386f32, 0.0005f32, 0f32, 0f32],
    [0.0166f32, -0.6235f32, 1.1089f32, -0.457f32, -0.0454f32, 0.001f32, 0f32, 0f32],
    [0.0176f32, -0.6187f32, 1.0879f32, -0.4346f32, -0.0532f32, 0.001f32, 0f32, 0f32],
    [0.0181f32, -0.6138f32, 1.0659f32, -0.4102f32, -0.0615f32, 0.0015f32, 0f32, 0f32],
    [0.019f32, -0.6069f32, 1.0405f32, -0.3843f32, -0.0698f32, 0.0015f32, 0f32, 0f32],
    [0.0195f32, -0.6006f32, 1.0161f32, -0.3574f32, -0.0796f32, 0.002f32, 0f32, 0f32],
    [0.02f32, -0.5928f32, 0.9893f32, -0.3286f32, -0.0898f32, 0.0024f32, 0f32, 0f32],
    [0.02f32, -0.582f32, 0.958f32, -0.2988f32, -0.1001f32, 0.0029f32, 0f32, 0f32],
    [0.02f32, -0.5728f32, 0.9292f32, -0.269f32, -0.1104f32, 0.0034f32, 0f32, 0f32],
    [0.02f32, -0.562f32, 0.8975f32, -0.2368f32, -0.1226f32, 0.0039f32, 0f32, 0f32],
    [0.0205f32, -0.5498f32, 0.8643f32, -0.2046f32, -0.1343f32, 0.0044f32, 0f32, 0f32],
    [0.02f32, -0.5371f32, 0.8301f32, -0.1709f32, -0.1465f32, 0.0049f32, 0f32, 0f32],
    [0.0195f32, -0.5239f32, 0.7944f32, -0.1367f32, -0.1587f32, 0.0054f32, 0f32, 0f32],
    [0.0195f32, -0.5107f32, 0.7598f32, -0.1021f32, -0.1724f32, 0.0059f32, 0f32, 0f32],
    [0.019f32, -0.4966f32, 0.7231f32, -0.0649f32, -0.1865f32, 0.0063f32, 0f32, 0f32],
    [0.0186f32, -0.4819f32, 0.6846f32, -0.0288f32, -0.1997f32, 0.0068f32, 0f32, 0f32],
    [0.0186f32, -0.4668f32, 0.646f32, 0.0093f32, -0.2144f32, 0.0073f32, 0f32, 0f32],
    [0.0176f32, -0.4507f32, 0.6055f32, 0.0479f32, -0.229f32, 0.0083f32, 0f32, 0f32],
    [0.0171f32, -0.437f32, 0.5693f32, 0.0859f32, -0.2446f32, 0.0088f32, 0f32, 0f32],
    [0.0161f32, -0.4199f32, 0.5283f32, 0.1255f32, -0.2598f32, 0.0098f32, 0f32, 0f32],
    [0.0161f32, -0.4048f32, 0.4883f32, 0.1655f32, -0.2754f32, 0.0103f32, 0f32, 0f32],
    [0.0151f32, -0.3887f32, 0.4497f32, 0.2041f32, -0.291f32, 0.0107f32, 0f32, 0f32],
    [0.0142f32, -0.3711f32, 0.4072f32, 0.2446f32, -0.3066f32, 0.0117f32, 0f32, 0f32],
    [0.0137f32, -0.3555f32, 0.3672f32, 0.2852f32, -0.3228f32, 0.0122f32, 0f32, 0f32],
    [0.0132f32, -0.3394f32, 0.3262f32, 0.3262f32, -0.3394f32, 0.0132f32, 0f32, 0f32],
    [0.0122f32, -0.3228f32, 0.2852f32, 0.3672f32, -0.3555f32, 0.0137f32, 0f32, 0f32],
    [0.0117f32, -0.3066f32, 0.2446f32, 0.4072f32, -0.3711f32, 0.0142f32, 0f32, 0f32],
    [0.0107f32, -0.291f32, 0.2041f32, 0.4497f32, -0.3887f32, 0.0151f32, 0f32, 0f32],
    [0.0103f32, -0.2754f32, 0.1655f32, 0.4883f32, -0.4048f32, 0.0161f32, 0f32, 0f32],
    [0.0098f32, -0.2598f32, 0.1255f32, 0.5283f32, -0.4199f32, 0.0161f32, 0f32, 0f32],
    [0.0088f32, -0.2446f32, 0.0859f32, 0.5693f32, -0.437f32, 0.0171f32, 0f32, 0f32],
    [0.0083f32, -0.229f32, 0.0479f32, 0.6055f32, -0.4507f32, 0.0176f32, 0f32, 0f32],
    [0.0073f32, -0.2144f32, 0.0093f32, 0.646f32, -0.4668f32, 0.0186f32, 0f32, 0f32],
    [0.0068f32, -0.1997f32, -0.0288f32, 0.6846f32, -0.4819f32, 0.0186f32, 0f32, 0f32],
    [0.0063f32, -0.1865f32, -0.0649f32, 0.7231f32, -0.4966f32, 0.019f32, 0f32, 0f32],
    [0.0059f32, -0.1724f32, -0.1021f32, 0.7598f32, -0.5107f32, 0.0195f32, 0f32, 0f32],
    [0.0054f32, -0.1587f32, -0.1367f32, 0.7944f32, -0.5239f32, 0.0195f32, 0f32, 0f32],
    [0.0049f32, -0.1465f32, -0.1709f32, 0.8301f32, -0.5371f32, 0.02f32, 0f32, 0f32],
    [0.0044f32, -0.1343f32, -0.2046f32, 0.8643f32, -0.5498f32, 0.0205f32, 0f32, 0f32],
    [0.0039f32, -0.1226f32, -0.2368f32, 0.8975f32, -0.562f32, 0.02f32, 0f32, 0f32],
    [0.0034f32, -0.1104f32, -0.269f32, 0.9292f32, -0.5728f32, 0.02f32, 0f32, 0f32],
    [0.0029f32, -0.1001f32, -0.2988f32, 0.958f32, -0.582f32, 0.02f32, 0f32, 0f32],
    [0.0024f32, -0.0898f32, -0.3286f32, 0.9893f32, -0.5928f32, 0.02f32, 0f32, 0f32],
    [0.002f32, -0.0796f32, -0.3574f32, 1.0161f32, -0.6006f32, 0.0195f32, 0f32, 0f32],
    [0.0015f32, -0.0698f32, -0.3843f32, 1.0405f32, -0.6069f32, 0.019f32, 0f32, 0f32],
    [0.0015f32, -0.0615f32, -0.4102f32, 1.0659f32, -0.6138f32, 0.0181f32, 0f32, 0f32],
    [0.001f32, -0.0532f32, -0.4346f32, 1.0879f32, -0.6187f32, 0.0176f32, 0f32, 0f32],
    [0.001f32, -0.0454f32, -0.457f32, 1.1089f32, -0.6235f32, 0.0166f32, 0f32, 0f32],
    [0.0005f32, -0.0386f32, -0.479f32, 1.1274f32, -0.6265f32, 0.0156f32, 0f32, 0f32],
    [0.0005f32, -0.0317f32, -0.5005f32, 1.1455f32, -0.6284f32, 0.0142f32, 0f32, 0f32],
    [0.0005f32, -0.0259f32, -0.5181f32, 1.1582f32, -0.627f32, 0.0122f32, 0f32, 0f32],
    [0.0005f32, -0.0195f32, -0.5352f32, 1.1699f32, -0.6265f32, 0.0112f32, 0f32, 0f32],
    [0f32, -0.0151f32, -0.5513f32, 1.1802f32, -0.6235f32, 0.0093f32, 0f32, 0f32],
    [0f32, -0.0103f32, -0.5659f32, 1.189f32, -0.6196f32, 0.0073f32, 0f32, 0f32],
    [0.0005f32, -0.0068f32, -0.5791f32, 1.1958f32, -0.6147f32, 0.0049f32, 0f32, 0f32],
    [0f32, -0.0029f32, -0.5903f32, 1.1987f32, -0.6084f32, 0.0029f32, 0f32, 0f32],
];

/// Return a reference to the NIS coef_scale table.
fn nis_coef_scale() -> &'static [[f32; 8]; 64] {
    &NIS_COEF_SCALE
}

/// Return a reference to the NIS coef_usm table.
fn nis_coef_usm() -> &'static [[f32; 8]; 64] {
    &NIS_COEF_USM
}
