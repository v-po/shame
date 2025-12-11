#![allow(clippy::collapsible_match)]

use std::f32::consts::TAU;
use std::ops::Range;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use shame::results::RenderPipeline;
use shame::TextureFormat;
use sm::results::BindingType;
use thiserror::Error;

use wgpu::util::DeviceExt as _;
use wgpu::{Color, LoadOp, StoreOp};

use shame_wgpu::{self as sm, SurfaceFormat};
use sm::texture_view::TextureViewExt;
use sm::aliases::*;
use sm::prelude::*;
use winit::event::{ElementState, MouseButton, WindowEvent};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    ShameWgpu(#[from] shame_wgpu::Error),
    #[error("{0}")]
    Pipeline(String),
}

pub struct HelloTriangle {
    shame_pipeline: shame::results::RenderPipeline,
    pipeline: wgpu::RenderPipeline,
    start_time: std::time::Instant,
    aspect_ratio: f32,

    // Camera State
    rot_x: f32, // Yaw
    rot_y: f32, // Pitch
    mouse_pressed: bool,
    last_mouse_pos: Option<(f64, f64)>,
}

impl HelloTriangle {
    pub fn new(gpu: &sm::Gpu) -> Result<Self, Error> {
        let surface_format = gpu.surface_format();
        let shame_pipe = pipeline::make_pipeline().unwrap();
        let pipeline = shame_wgpu::conversion::render_pipeline(gpu, shame_pipe.clone(), surface_format).unwrap();

        Ok(Self {
            shame_pipeline: shame_pipe,
            pipeline,
            start_time: std::time::Instant::now(),
            aspect_ratio: 1.0,
            rot_x: 0.0,
            rot_y: 0.5,
            mouse_pressed: false,
            last_mouse_pos: None,
        })
    }

    fn interface_compatible(&self, new_pdef: &sm::results::RenderPipeline) -> bool {
        let old_info = &self.shame_pipeline.pipeline;
        let new_info = &new_pdef.pipeline;

        if old_info.push_constants.push_constants_byte_size != new_info.push_constants.push_constants_byte_size {
            eprintln!("[hot_reload] mismatch: Push constant size changed.");
            return false;
        }

        fn range_eq(a: &Option<Range<u32>>, b: &Option<Range<u32>>) -> bool {
            match (a, b) {
                (Some(ra), Some(rb)) => ra == rb,
                (None, None) => true,
                _ => false,
            }
        }
        if !range_eq(&old_info.push_constants.vert, &new_info.push_constants.vert) {
            return false;
        }
        if !range_eq(&old_info.push_constants.frag, &new_info.push_constants.frag) {
            return false;
        }

        if old_info.bind_groups.len() != new_info.bind_groups.len() {
            eprintln!("[hot_reload] mismatch: Bind group count changed.");
            return false;
        }

        for (idx, old_layout) in &old_info.bind_groups {
            let Some(new_layout) = new_info.bind_groups.get(idx) else {
                return false;
            };

            if old_layout.bindings.len() != new_layout.bindings.len() {
                return false;
            }

            for (b_idx, old_binding) in &old_layout.bindings {
                let Some(new_binding) = new_layout.bindings.get(b_idx) else {
                    return false;
                };

                if !binding_types_compatible(&old_binding.binding_ty, &new_binding.binding_ty) {
                    eprintln!("[hot_reload] mismatch: Binding type at {}:{} changed.", idx, b_idx);
                    return false;
                }
            }
        }

        if old_info.vertex_buffers.len() != new_info.vertex_buffers.len() {
            return false;
        }

        // TODO: vertex buffers

        true
    }

    pub fn try_replace_pipeline(
        &mut self,
        pipeline: shame::results::RenderPipeline,
        gpu: &sm::Gpu,
    ) -> Result<(), Error> {
        if (!self.interface_compatible(&pipeline)) {
            return Err(Error::Pipeline("pipeline interface mismatch".to_string()));
        }
        let surface_format = gpu.surface_format();
        let new_pipeline = shame_wgpu::conversion::render_pipeline(gpu, pipeline, surface_format).unwrap();
        self.pipeline = new_pipeline;
        Ok(())
    }

    pub fn submit_render_commands_to_gpu(&mut self, gpu: &sm::Gpu, surface: &wgpu::TextureView) -> Result<(), Error> {
        let time = self.start_time.elapsed().as_secs_f32();

        let mut cmd = gpu.create_command_encoder(&Default::default());
        {
            let mut pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[
                    surface.attach_as_color(wgpu::LoadOp::Clear(wgpu::Color::BLACK), wgpu::StoreOp::Store)
                ],
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);

            let pc_data = [time, self.aspect_ratio, self.rot_x, self.rot_y];
            let pc_bytes = bytemuck::cast_slice(&pc_data);

            pass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, pc_bytes);
            pass.draw(0..3, 0..1);
        }

        let _ticket = gpu.queue().submit([cmd.finish()]);
        gpu.poll(wgpu::PollType::Poll);
        Ok(())
    }

    pub fn window_event(&mut self, event: &WindowEvent) -> Result<(), Error> {
        match event {
            WindowEvent::Resized(size) => {
                if size.height > 0 {
                    self.aspect_ratio = size.width as f32 / size.height as f32;
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x, position.y);
                if self.mouse_pressed {
                    if let Some((prev_x, prev_y)) = self.last_mouse_pos {
                        let delta_x = x - prev_x;
                        let delta_y = y - prev_y;

                        let s = 0.01;
                        self.rot_x -= delta_x as f32 * s;
                        self.rot_y -= delta_y as f32 * s;
                        self.rot_y = self.rot_y.clamp(-1.5, 1.5);
                    }
                }
                self.last_mouse_pos = Some((x, y));
            }
            _ => {}
        }
        Ok(())
    }
}

fn binding_types_compatible(a: &BindingType, b: &BindingType) -> bool {
    use sm::results::BindingType;
    use sm::results::BufferBindingType;

    match (a, b) {
        (BindingType::Buffer { ty: t1, .. }, BindingType::Buffer { ty: t2, .. }) => {
            matches!(
                (t1, t2),
                (BufferBindingType::Uniform, BufferBindingType::Uniform)
                    | (BufferBindingType::Storage(_), BufferBindingType::Storage(_))
            )
        }
        (BindingType::Sampler(s1), BindingType::Sampler(s2)) => true,
        (BindingType::SampledTexture { .. }, BindingType::SampledTexture { .. }) => true,
        (BindingType::StorageTexture { .. }, BindingType::StorageTexture { .. }) => true,
        _ => false,
    }
}
