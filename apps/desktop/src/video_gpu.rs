//! Colour conversion for watched streams on the GPU. Decoded pictures arrive as I420 planes
//! (1.5 bytes a pixel); they are uploaded as three single-channel textures and a small shader
//! writes RGBA into a texture the interface draws like any other image. The CPU only copies
//! bytes: no per-pixel colour math, and 2.7× less data crosses to the GPU than packed RGBA.
use eframe::{egui, egui_wgpu, wgpu};

/// The conversion shader; `crates/solace-ui/tests/shaders.rs` validates it in CI.
const SHADER: &str = include_str!("yuv.wgsl");

/// Textures sized for one picture size; replaced when the stream changes resolution.
struct Target {
	size: [u32; 2],
	planes: [wgpu::Texture; 3],
	bind_group: wgpu::BindGroup,
	output: wgpu::Texture,
	/// Render target for the shader and image for the interface; registered once.
	view: wgpu::TextureView,
	id: Option<egui::TextureId>,
}

pub struct VideoGpu {
	state: egui_wgpu::RenderState,
	pipeline: wgpu::RenderPipeline,
	layout: wgpu::BindGroupLayout,
	sampler: wgpu::Sampler,
	target: Option<Target>,
}

impl VideoGpu {
	pub fn new(state: egui_wgpu::RenderState) -> Self {
		let device = &state.device;
		let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("solace-yuv"),
			source: wgpu::ShaderSource::Wgsl(SHADER.into()),
		});
		let plane = |binding| wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable: true },
				view_dimension: wgpu::TextureViewDimension::D2,
				multisampled: false,
			},
			count: None,
		};
		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("solace-yuv"),
			entries: &[
				plane(0),
				plane(1),
				plane(2),
				wgpu::BindGroupLayoutEntry {
					binding: 3,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("solace-yuv"),
			bind_group_layouts: &[Some(&layout)],
			immediate_size: 0,
		});
		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("solace-yuv"),
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &module,
				entry_point: Some("vs"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &module,
				entry_point: Some("fs"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::Rgba8Unorm,
					blend: None,
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState::default(),
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("solace-yuv"),
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		Self {
			state,
			pipeline,
			layout,
			sampler,
			target: None,
		}
	}

	/// Upload one I420 picture and convert it; returns the texture the interface should draw.
	pub fn show(&mut self, width: u32, height: u32, i420: &[u8]) -> Option<egui::TextureId> {
		let (w, h) = (width as usize, height as usize);
		if width == 0 || height == 0 || i420.len() != platform::video::i420_len(w, h) {
			return None;
		}
		if self
			.target
			.as_ref()
			.is_none_or(|t| t.size != [width, height])
		{
			self.replace_target(width, height);
		}
		let target = self.target.as_mut()?;
		let (cw, ch) = platform::video::chroma_size(w, h);
		let (y, chroma) = i420.split_at(w * h);
		let (u, v) = chroma.split_at(cw * ch);
		let queue = &self.state.queue;
		for (texture, bytes, pw, ph) in [
			(&target.planes[0], y, w, h),
			(&target.planes[1], u, cw, ch),
			(&target.planes[2], v, cw, ch),
		] {
			queue.write_texture(
				texture.as_image_copy(),
				bytes,
				wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(pw as u32),
					rows_per_image: Some(ph as u32),
				},
				wgpu::Extent3d {
					width: pw as u32,
					height: ph as u32,
					depth_or_array_layers: 1,
				},
			);
		}
		let mut encoder =
			self.state
				.device
				.create_command_encoder(&wgpu::CommandEncoderDescriptor {
					label: Some("solace-yuv"),
				});
		{
			let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("solace-yuv"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &target.view,
					depth_slice: None,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
				multiview_mask: None,
			});
			pass.set_pipeline(&self.pipeline);
			pass.set_bind_group(0, &target.bind_group, &[]);
			pass.draw(0..3, 0..1);
		}
		// Submitted before egui's own frame, so the picture is ready when the UI samples it.
		queue.submit([encoder.finish()]);
		if target.id.is_none() {
			target.id = Some(self.state.renderer.write().register_native_texture(
				&self.state.device,
				&target.view,
				wgpu::FilterMode::Linear,
			));
		}
		target.id
	}

	fn replace_target(&mut self, width: u32, height: u32) {
		self.release();
		let device = &self.state.device;
		let (cw, ch) = platform::video::chroma_size(width as usize, height as usize);
		let plane = |w: u32, h: u32| {
			device.create_texture(&wgpu::TextureDescriptor {
				label: Some("solace-yuv-plane"),
				size: wgpu::Extent3d {
					width: w,
					height: h,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::R8Unorm,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				view_formats: &[],
			})
		};
		let planes = [
			plane(width, height),
			plane(cw as u32, ch as u32),
			plane(cw as u32, ch as u32),
		];
		let views: Vec<wgpu::TextureView> = planes
			.iter()
			.map(|p| p.create_view(&wgpu::TextureViewDescriptor::default()))
			.collect();
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("solace-yuv"),
			layout: &self.layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&views[0]),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&views[1]),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&views[2]),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
		});
		// egui samples image textures as gamma-encoded `Rgba8Unorm`, which is what the shader
		// writes, so the stream matches every other image on screen.
		let output = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("solace-stream"),
			size: wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});
		let view = output.create_view(&wgpu::TextureViewDescriptor::default());
		self.target = Some(Target {
			size: [width, height],
			planes,
			bind_group,
			output,
			view,
			id: None,
		});
	}

	/// Free the GPU textures (the stream ended or stopped).
	pub fn release(&mut self) {
		if let Some(target) = self.target.take() {
			if let Some(id) = target.id {
				self.state.renderer.write().free_texture(&id);
			}
			for plane in &target.planes {
				plane.destroy();
			}
			target.output.destroy();
		}
	}
}

impl Drop for VideoGpu {
	fn drop(&mut self) {
		self.release();
	}
}
