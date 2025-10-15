#![allow(clippy::collapsible_match)]

use std::f32::consts::TAU;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use shame::results::RenderPipeline;
use shame::TextureFormat;
use thiserror::Error;

use wgpu::util::DeviceExt as _;
use wgpu::{LoadOp::*, StoreOp::*};

use shame_wgpu::{self as sm, SurfaceFormat};
use sm::texture_view::TextureViewExt;
use sm::aliases::*;
use sm::prelude::*;

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
    time: f32,
}

impl HelloTriangle {
    pub fn new(gpu: &sm::Gpu) -> Result<Self, Error> {
        let surface_format = gpu.surface_format();
        let shame_pipeline: shame::results::RenderPipeline = pipeline::make_pipeline().unwrap();
        let pipeline: wgpu::RenderPipeline =
            shame_wgpu::conversion::render_pipeline(gpu, shame_pipeline.clone(), surface_format).unwrap();

        Ok(Self {
            shame_pipeline,
            pipeline,
            time: 0.0,
        })
    }

    pub fn try_replace_pipeline(&mut self, pipeline: sm::results::RenderPipeline, gpu: &sm::Gpu) -> Result<(), Error> {
        if !self.interface_compatible(&pipeline) {
            return Err(Error::Pipeline("pipeline rejected (interface mismatch)".to_string()));
        }

        let surface_format = gpu.surface_format();
        let new_pipeline = shame_wgpu::conversion::render_pipeline(gpu, pipeline, surface_format).unwrap();

        self.pipeline = new_pipeline;
        Ok(())
    }

    fn interface_compatible(&self, new_pdef: &sm::results::RenderPipeline) -> bool {
        true
        // self.shame_pipeline.pipeline == new_pdef.pipeline
    }

    pub fn submit_render_commands_to_gpu(&mut self, gpu: &sm::Gpu, surface: &wgpu::TextureView) -> Result<(), Error> {
        self.time += 1.0 / 60.0;

        let mut cmd = gpu.create_command_encoder(&Default::default());
        {
            let mut pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[surface.attach_as_color(Clear(wgpu::Color::BLACK), Store)],
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);

            pass.set_push_constants(
                // in the future shame_wgpu can add a wgpu pipeline wrapper that
                // automatically sets this correctly, since shame returns all the
                // relevant info.
                // For now a wgpu validation error is triggered if the stages don't match
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::bytes_of(&self.time),
            );
            pass.draw(0..3, 0..5);
        }

        let _ticket = gpu.queue().submit([cmd.finish()]);

        gpu.poll(wgpu::PollType::Poll);
        Ok(())
    }

    pub fn window_event(&mut self, event: &winit::event::WindowEvent) -> Result<(), Error> {
        Ok(())
    }
}
