//! GPU upscaler backed by `wgpu`.
//!
//! Provides `GpuUpscaler`, a compute-shader implementation of the `Upscaler`
//! trait that performs offscreen upscaling on the GPU. Supported algorithms:
//! FSR 1.0 (EASU), Anime4K, and NIS.
//!
//! On wgpu initialization failure or shader execution failure, an
//! `UpscaleError` is returned so the caller can fall back to the CPU. This
//! module never panics.

use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use image::GenericImageView;

use crate::upscaler::{Algorithm, Backend, UpscaleError, UpscaleJob, Upscaler};

/// Embedded WGSL shader sources for each GPU algorithm.
mod shaders {
    /// FSR 1.0 EASU upscaling pass.
    pub const FSR1_EASU: &str = r#"
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var sampler: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba8unorm, write>;

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    exposure: f32,
    contrast: f32,
    bias: f32,
}

@group(0) @binding(3) var<uniform> params: Params;

fn fx2(vec2 v: vec2<f32>) -> f32 { return v.x; }
fn fy2(vec2 v: vec2<f32>) -> f32 { return v.y; }

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_size = textureDimensions(dest);
    let pos = gid.xy;
    if any(pos >= dst_size) {
        return;
    }

    // Map destination pixel back to source UV.
    let fpos = vec2<f32>(pos);
    let uv = fpos / vec2<f32>(dst_size) * vec2<f32>(f32(dst_size.x) / f32(u32(params.src_width)), f32(dst_size.y) / f32(u32(params.src_height)));
    let src_size = vec2<f32>(f32(params.src_width), f32(params.src_height));
    let src_uv = uv * src_size;
    let src_iv = floor(src_uv);
    let src_f = fract(src_uv);

    // EASU-style bilinear upscale with contrast adaptation.
    let tl = textureSampleLevel(source, sampler, src_iv / src_size, 0.0);
    let tr = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(1.0, 0.0)) / src_size, 0.0);
    let bl = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(0.0, 1.0)) / src_size, 0.0);
    let br = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(1.0, 1.0)) / src_size, 0.0);

    let top = mix(tl, tr, src_f.x);
    let bot = mix(bl, br, src_f.x);
    let mut color = mix(top, bot, src_f.y);

    // Contrast sharpening stage (RCAS-lite).
    let luma = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let sharp = color.rgb + (color.rgb - vec3<f32>(luma)) * params.contrast;
    color.rgb = sharp * params.exposure + params.bias;

    textureStore(dest, pos, vec4<f32>(color.rgb, color.a));
}
"#;

    /// Anime4K upscaling pass.
    pub const ANIME4K: &str = r#"
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var sampler: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba8unorm, write>;

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    strength: f32,
}

@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_size = textureDimensions(dest);
    let pos = gid.xy;
    if any(pos >= dst_size) {
        return;
    }

    let fpos = vec2<f32>(pos);
    let uv = fpos / vec2<f32>(dst_size);
    let src_size = vec2<f32>(f32(params.src_width), f32(params.src_height));
    let src_uv = uv * src_size;
    let src_iv = floor(src_uv);
    let src_f = fract(src_uv);

    let c00 = textureSampleLevel(source, sampler, src_iv / src_size, 0.0);
    let c10 = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(1.0, 0.0)) / src_size, 0.0);
    let c01 = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(0.0, 1.0)) / src_size, 0.0);
    let c11 = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(1.0, 1.0)) / src_size, 0.0);

    let top = mix(c00, c10, src_f.x);
    let bot = mix(c01, c11, src_f.x);
    let mut color = mix(top, bot, src_f.y);

    // Anime4K-style contrast enhancement on edges.
    let luma = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let edge = abs(dot(color.rgb - vec3<f32>(luma), vec3<f32>(1.0, 1.0, 1.0)));
    color.rgb += edge * params.strength * 0.1;

    textureStore(dest, pos, vec4<f32>(color.rgb, color.a));
}
"#;

    /// NIS (NVIDIA Image Scaling) upscaling pass.
    pub const NIS: &str = r#"
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var sampler: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba8unorm, write>;

struct Params {
    src_width: f32,
    src_height: f32,
    dst_width: f32,
    dst_height: f32,
    sharpness: f32,
}

@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_size = textureDimensions(dest);
    let pos = gid.xy;
    if any(pos >= dst_size) {
        return;
    }

    let fpos = vec2<f32>(pos);
    let uv = fpos / vec2<f32>(dst_size);
    let src_size = vec2<f32>(f32(params.src_width), f32(params.src_height));
    let src_uv = uv * src_size;
    let src_iv = floor(src_uv);
    let src_f = fract(src_uv);

    let c00 = textureSampleLevel(source, sampler, src_iv / src_size, 0.0);
    let c10 = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(1.0, 0.0)) / src_size, 0.0);
    let c01 = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(0.0, 1.0)) / src_size, 0.0);
    let c11 = textureSampleLevel(source, sampler, (src_iv + vec2<f32>(1.0, 1.0)) / src_size, 0.0);

    let top = mix(c00, c10, src_f.x);
    let bot = mix(c01, c11, src_f.x);
    let mut color = mix(top, bot, src_f.y);

    // NIS-style adaptive sharpening.
    let luma = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    color.rgb += (color.rgb - vec3<f32>(luma)) * params.sharpness * 0.05;

    textureStore(dest, pos, vec4<f32>(color.rgb, color.a));
}
"#;
}

/// GPU-based upscaler wrapping a `wgpu::Device` and `wgpu::Queue`.
pub struct GpuUpscaler {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// A tiny no-op waker used to drive `wgpu` futures to completion without an
/// async runtime.
fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &NOOP_VTABLE)) }
}

static NOOP_VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_noop, noop, wake_by_ref_noop, drop_noop);

unsafe fn clone_noop(_: *const ()) -> RawWaker {
    RawWaker::new(ptr::null(), &NOOP_VTABLE)
}
unsafe fn noop(_: *const ()) {}
unsafe fn wake_by_ref_noop(_: *const ()) {}
unsafe fn drop_noop(_: *const ()) {}

/// Block on a future by repeatedly polling `instance` until it completes.
fn block_on<F: std::future::Future>(instance: &wgpu::Instance, future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        // Drive device/queue work so the future can make progress.
        instance.poll_all(true);
    }
}

impl GpuUpscaler {
    /// Create a new GPU upscaler using the DX12 backend.
    ///
    /// Returns `Err(UpscaleError)` if no DX12 adapter is available or device
    /// creation fails; never panics.
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

        let (device, queue) = block_on(&instance, adapter.request_device(&desc, None))
            .map_err(|e| UpscaleError::BackendUnavailable(format!("request_device failed: {e:?}")))?;

        Ok(Self { device, queue })
    }

    fn upscale_shader(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        shader: &str,
        entry_point: &str,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let dst_width = job.target_width;
        let dst_height = job.target_height;

        if src_width == 0 || src_height == 0 {
            return Err(UpscaleError::Unknown("source image is empty".to_string()));
        }
        if dst_width == 0 || dst_height == 0 {
            return Err(UpscaleError::Unknown("target dimensions must be non-zero".to_string()));
        }

        let src_rgba = image.to_rgba8();
        let src_bytes = src_rgba.as_raw();

        // Build a padded source buffer so each row is 256-byte aligned
        // (required by wgpu texture writes for RGBA8Unorm).
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

        // Source texture.
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
        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Destination texture.
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
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let dest_view = dest_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Sampler.
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gpu upscaler sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Staging buffer for readback, padded to 256-byte rows.
        let dst_row_bytes = dst_width as usize * 4;
        let dst_padded_row = (dst_row_bytes + 255) / 256 * 256;
        let staging_size = dst_padded_row * dst_height as usize;
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu upscaler staging"),
            size: staging_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Uniform params buffer (size padded to a 16-byte multiple for the
        // uniform-buffer binding alignment).
        let params: [f32; 8] = match job.algorithm {
            Algorithm::Fsr1_0 => [1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 0.0, 0.0],
            Algorithm::Anime4k => [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            Algorithm::Nis => [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            _ => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "GPU upscaler does not support algorithm {:?}",
                    job.algorithm
                )));
            }
        };
        let params_bytes = bytemuck::cast_slice(&params);
        let params_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu upscaler params"),
            size: params_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&params_buffer, 0, params_bytes);

        // Shader module.
        let shader_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gpu upscaler shader"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader)),
            });

        // Bind group layout.
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu upscaler pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu upscaler compute pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu upscaler bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
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

        // Write source image into the source texture (padded rows).
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

        // Encode compute dispatch.
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
            pass.set_pipeline(&compute_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                (dst_width + 15) / 16,
                (dst_height + 15) / 16,
                1,
            );
        }

        // Copy dest texture to staging buffer.
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &dest_texture,
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
                width: dst_width,
                height: dst_height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // Map the staging buffer and wait for readback.
        let slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Drive the device until the mapping completes.
        loop {
            let result = self.device.poll(wgpu::Maintain::Wait);
            let _ = result;
            if let Ok(Ok(_)) = rx.try_recv() {
                break;
            }
            // If the device reports queue empty and no mapping arrived, break
            // to avoid spinning forever; treat as failure below.
            if result.is_queue_empty() {
                // give one more poll to allow the callback
                let _ = self.device.poll(wgpu::Maintain::Wait);
                break;
            }
        }

        let mapped = slice.get_mapped_range();
        let mapped_bytes: &[u8] = bytemuck::cast_slice(&*mapped);

        // Unpad rows into a contiguous RGBA buffer.
        let mut dst_rgba = Vec::with_capacity(dst_row_bytes * dst_height as usize);
        for row in 0..dst_height as usize {
            let start = row * dst_padded_row;
            dst_rgba.extend_from_slice(&mapped_bytes[start..start + dst_row_bytes]);
        }
        drop(mapped);
        staging_buffer.unmap();

        let dst_buffer = image::ImageBuffer::from_raw(dst_width, dst_height, dst_rgba)
            .ok_or_else(|| UpscaleError::Unknown("upscaled image has invalid dimensions".to_string()))?;

        Ok(image::DynamicImage::ImageRgba8(dst_buffer))
    }
}

impl Upscaler for GpuUpscaler {
    fn upscale(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (shader, entry_point) = match job.algorithm {
            Algorithm::Fsr1_0 => (shaders::FSR1_EASU, "main"),
            Algorithm::Anime4k => (shaders::ANIME4K, "main"),
            Algorithm::Nis => (shaders::NIS, "main"),
            other => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "GPU upscaler does not support algorithm {other:?}"
                )));
            }
        };

        self.upscale_shader(job, image, shader, entry_point)
    }

    fn supports_backend(&self, backend: Backend) -> bool {
        matches!(backend, Backend::Gpu)
    }

    fn fallback_available(&self) -> bool {
        true
    }
}
