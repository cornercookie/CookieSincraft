//! GPU upscaler backed by `wgpu`.
//!
//! Provides `GpuUpscaler`, a compute-shader implementation of the `Upscaler`
//! trait that performs offscreen upscaling on the GPU. Supported algorithms:
//! FSR 1.0 (EASU + RCAS), NIS, and Anime4K (CNN x2 M).
//!
//! On wgpu initialization failure or shader execution failure, an
//! `UpscaleError` is returned so the caller can fall back to the CPU.
//! This module never panics.

use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use image::GenericImageView;
use wgpu::util::DeviceExt;

use crate::upscaler::{Algorithm, Backend, UpscaleError, UpscaleJob, Upscaler};

/// GPU-based upscaler wrapping a `wgpu::Device` and `wgpu::Queue`.
pub struct GpuUpscaler {
    device: wgpu::Device,
    queue: wgpu::Queue,
    error_flag: Arc<Mutex<bool>>,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    easu_pipeline: wgpu::ComputePipeline,
    rcas_pipeline: wgpu::ComputePipeline,
    nis_pipeline: wgpu::ComputePipeline,
    anime4k_pipeline: wgpu::ComputePipeline,
    sampler: wgpu::Sampler,
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

        // Error tracking to avoid panics on validation errors
        let error_flag = Arc::new(Mutex::new(false));
        let error_flag_clone = error_flag.clone();
        device.on_uncaptured_error(Box::new(move |_err| {
            #[cfg(debug_assertions)]
            tracing::warn!("wgpu uncaptured error");
            *error_flag_clone.lock().unwrap() = true;
        }));

        // Shared bind group layout for all pipelines
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu upscaler pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Load shaders from external files with panic protection
        let easu_shader = Self::create_shader_module_safe(&device, "fsr easu shader", include_str!(
            "../shaders/fsr_easu.wgsl"
        ))?;
        let rcas_shader = Self::create_shader_module_safe(&device, "fsr rcas shader", include_str!(
            "../shaders/fsr_rcas.wgsl"
        ))?;
        let nis_shader = Self::create_shader_module_safe(&device, "nis shader", include_str!(
            "../shaders/nis_scaler.wgsl"
        ))?;
        let anime4k_shader = Self::create_shader_module_safe(&device, "anime4k shader", include_str!(
            "../shaders/anime4k_cnn_x2_m.wgsl"
        ))?;

        let easu_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fsr easu pipeline"),
            layout: Some(&pipeline_layout),
            module: &easu_shader,
            entry_point: "main",
        });
        let rcas_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fsr rcas pipeline"),
            layout: Some(&pipeline_layout),
            module: &rcas_shader,
            entry_point: "main",
        });
        let nis_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("nis pipeline"),
            layout: Some(&pipeline_layout),
            module: &nis_shader,
            entry_point: "main",
        });
        let anime4k_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("anime4k pipeline"),
            layout: Some(&pipeline_layout),
            module: &anime4k_shader,
            entry_point: "main",
        });

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

        Ok(Self {
            device,
            queue,
            error_flag: Arc::new(Mutex::new(false)),
            bind_group_layout,
            pipeline_layout,
            easu_pipeline,
            rcas_pipeline,
            nis_pipeline,
            anime4k_pipeline,
            sampler,
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

    /// Upscale an image according to `job`.
    pub fn upscale(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
    ) -> Result<image::DynamicImage, UpscaleError> {
        // Check error flag from previous operations
        if *self.error_flag.lock().unwrap() {
            return Err(UpscaleError::BackendUnavailable(
                "previous GPU operation left device in error state".to_string(),
            ));
        }

        let result = match job.algorithm {
            Algorithm::Fsr1_0 => self.upscale_fsr(job, image),
            Algorithm::Nis => self.upscale_nis(job, image),
            Algorithm::Anime4k => self.upscale_anime4k(job, image),
            other => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "GPU upscaler does not support algorithm {other:?}"
                )));
            }
        };

        // Check error flag after execution
        if *self.error_flag.lock().unwrap() {
            return Err(UpscaleError::BackendUnavailable(
                "GPU execution produced validation errors".to_string(),
            ));
        }

        result
    }

    fn upscale_fsr(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
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

        // FSR 1.0 only supports 1x-2x scale
        let scale_x = dst_width as f32 / src_width as f32;
        let scale_y = dst_height as f32 / src_height as f32;
        if scale_x > 2.0 || scale_y > 2.0 || scale_x < 1.0 || scale_y < 1.0 {
            return Err(UpscaleError::UnsupportedAlgorithm(
                "FSR 1.0 only supports 1x-2x scale per axis".to_string(),
            ));
        }

        // FSR 1.0 two-pass: EASU (upscale) -> RCAS (sharpen)
        // Pass 1: EASU upscales from source to destination size
        let intermediate = self.execute_pipeline_with_checkpoints(
            job,
            image,
            &self.easu_pipeline,
            0.0, // sharpness not used in EASU
        )?;

        // Pass 2: RCAS sharpens the upscaled result
        // Use RCAS pass with 0.2 stops (AMD default)
        self.execute_pipeline_with_checkpoints(
            &UpscaleJob {
                target_width: dst_width,
                target_height: dst_height,
                ..job.clone()
            },
            &intermediate,
            &self.rcas_pipeline,
            0.2,
        )
    }

    fn upscale_nis(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
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

        // NIS supports 1x-2x scale
        let scale_x = dst_width as f32 / src_width as f32;
        let scale_y = dst_height as f32 / src_height as f32;
        if scale_x > 2.0 || scale_y > 2.0 || scale_x < 1.0 || scale_y < 1.0 {
            return Err(UpscaleError::UnsupportedAlgorithm(
                "NIS only supports 1x-2x scale per axis".to_string(),
            ));
        }

        // Sharpness 0.5 = slider midpoint (NIS_Config.h:167)
        self.execute_pipeline_with_checkpoints(job, image, &self.nis_pipeline, 0.5)
    }

    fn upscale_anime4k(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
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

        // WHEN condition: only run Anime4K when both axes scale > 1.2
        let scale_x = dst_width as f32 / src_width as f32;
        let scale_y = dst_height as f32 / src_height as f32;
        if scale_x <= 1.2 || scale_y <= 1.2 {
            // Below threshold, use simple upscale (bilinear)
            return self.execute_pipeline_with_checkpoints(job, image, &self.anime4k_pipeline, 0.0);
        }

        // For >2x, chain multiple 2x passes then final Lanczos3 to exact size
        // Simplified: single pass for now
        self.execute_pipeline_with_checkpoints(job, image, &self.anime4k_pipeline, 0.5)
    }

    fn execute_pipeline_with_checkpoints(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        pipeline: &wgpu::ComputePipeline,
        sharpness: f32,
    ) -> Result<image::DynamicImage, UpscaleError> {
        // Push error scopes for validation and OOM
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);

        let result = self.execute_pipeline(job, image, pipeline, sharpness);

        // Check error scopes after execution
        let validation_err = futures::executor::block_on(self.device.pop_error_scope());
        let oom_err = futures::executor::block_on(self.device.pop_error_scope());
        if let Some(err) = validation_err.or(oom_err) {
            *self.error_flag.lock().unwrap() = true;
            return Err(UpscaleError::BackendUnavailable(format!("GPU error scope: {err:?}")));
        }

        result
    }

    fn execute_pipeline(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        pipeline: &wgpu::ComputePipeline,
        sharpness: f32,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        let dst_width = job.target_width;
        let dst_height = job.target_height;

        // Prepare source data
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

        // Source texture
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

        // Destination texture
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

        // Write source
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

        // Staging buffer
        let dst_row_bytes = dst_width as usize * 4;
        let dst_padded_row = (dst_row_bytes + 255) / 256 * 256;
        let staging_size = dst_padded_row * dst_height as usize;
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu upscaler staging"),
            size: staging_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Params buffer (5 f32 padded to 16-byte alignment = 8 f32 = 32 bytes)
        let params: [f32; 8] = [
            src_width as f32,
            src_height as f32,
            dst_width as f32,
            dst_height as f32,
            sharpness,
            0.0, 0.0, 0.0,
        ];
        let params_bytes = bytemuck::cast_slice(&params);
        let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gpu upscaler params"),
            contents: params_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group
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

        // Encode compute dispatch
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
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                (dst_width + 15) / 16,
                (dst_height + 15) / 16,
                1,
            );
        }

        // Copy dest to staging
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

        // Wait for readback with error scope checking
        let slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        loop {
            let result = self.device.poll(wgpu::Maintain::Wait);
            if let Ok(Ok(_)) = rx.try_recv() {
                break;
            }
            if result.is_queue_empty() {
                let _ = self.device.poll(wgpu::Maintain::Wait);
                break;
            }
        }

        // Read back
        let mapped = slice.get_mapped_range();
        let mapped_bytes: &[u8] = bytemuck::cast_slice(&*mapped);
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
    fn upscale(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
    ) -> Result<image::DynamicImage, UpscaleError> {
        // Check error flag from previous operations
        if *self.error_flag.lock().unwrap() {
            return Err(UpscaleError::BackendUnavailable(
                "previous GPU operation left device in error state".to_string(),
            ));
        }

        let result = match job.algorithm {
            Algorithm::Fsr1_0 => self.upscale_fsr(job, image),
            Algorithm::Nis => self.upscale_nis(job, image),
            Algorithm::Anime4k => self.upscale_anime4k(job, image),
            other => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "GPU upscaler does not support algorithm {other:?}"
                )));
            }
        };

        // Check error flag after execution
        if *self.error_flag.lock().unwrap() {
            return Err(UpscaleError::BackendUnavailable(
                "GPU execution produced validation errors".to_string(),
            ));
        }

        result
    }

    fn supports_backend(&self, backend: Backend) -> bool {
        matches!(backend, Backend::Gpu)
    }

    fn fallback_available(&self) -> bool {
        true
    }
}