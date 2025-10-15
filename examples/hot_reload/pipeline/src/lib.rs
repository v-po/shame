use std::f32::consts::TAU;

use shame::GpuLayout;
use shame as sm;
use sm::aliases::*;
use sm::prelude::*;
mod surface_format;
use surface_format::SurfaceFormat;

#[unsafe(no_mangle)]
pub extern "C" fn make_pipeline_ptr() -> *mut std::ffi::c_void {
    match make_pipeline() {
        Ok(p) => Box::into_raw(Box::new(p)) as *mut _,
        Err(_) => std::ptr::null_mut(),
    }
}

pub fn make_pipeline() -> Result<sm::results::RenderPipeline, sm::EncodingErrors> {
    p1()
    // p2()
}

fn p1() -> Result<sm::results::RenderPipeline, sm::EncodingErrors> {
    let mut encoder = sm::start_encoding(sm::Settings::default())?;

    let mut drawcall = encoder.new_render_pipeline(sm::Indexing::Incremental);

    let colors = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)].to_gpu();

    let index = drawcall.vertices.index;
    let vert_color = colors.at(index);

    let time: f32x1 = drawcall.push_constants.get();

    // let angle_offset = drawcall.vertices.instance_index.to_f32() * TAU / 5.0;
    //
    // let angle = -0.1f32.to_gpu() * time * 0.1 + angle_offset;
    // calculate equilateral triangle corner positions

    let id32 = index.to_u32();
    let u = ((id32 << 1) & 2).to_f32();
    let v = (id32 & 2).to_f32();

    let uv = sm::vec!(u, v);

    let pos = sm::vec!(uv * 2.0 - 1.0, 0.0, 1.0);

    // let uv: f32x2 = drawcall.vertices.buffers.next().index(index);

    let frag = drawcall
        .vertices
        .assemble(pos, sm::Draw::triangle_list(sm::Winding::Ccw))
        .rasterize(sm::Accuracy::default());

    let uv = frag.fill(uv);

    // grid logic
    let grid_scale = 10.0;
    let line_width = sm::vec!(0.02, 0.02);
    let uv_scaled = uv * grid_scale;

    let angle = -0.1f32.to_gpu() * time * 0.1; // radians
    let rot_z = sm::mat::from_rows([
        sm::vec!(angle.cos(), -angle.sin(), 0.0),
        sm::vec!(angle.sin(), angle.cos(), 0.0),
        sm::vec!(0.0, 0.0, 1.0),
    ]);
    let rot_x = sm::mat::from_rows([
        sm::vec!(1.0, 0.0, 0.0),
        sm::vec!(0.0, angle.cos(), -angle.sin()),
        sm::vec!(0.0, angle.sin(), angle.cos()),
    ]);
    let rot_y = sm::mat::from_rows([
        sm::vec!(angle.cos(), 0.0, angle.sin()),
        sm::vec!(0.0, 1.0, 0.0),
        sm::vec!(-angle.sin(), 0.0, angle.cos()),
    ]);

    let uv3 = sm::vec!(uv_scaled.x, uv_scaled.y, 0.0);
    let uv3_rotated = rot_x * uv3;
    // let uv_scaled = uv3_rotated.xy();

    let uv_scaled = uv3_rotated.xy() / (1.0 + uv3_rotated.z * 0.5);
    // let uv_scaled = rot * uv_scaled;

    // let offset = sm::vec!(0.0, 0.0);
    // let uv_transformed = (rot * (uv * grid_scale + offset)) / (1.0 + uv.x * 0.4);

    let duv = frag.quad.grad(uv_scaled, sm::GradPrecision::Fine);
    let uv_deriv = sm::vec!(duv.dx.length(), duv.dy.length());

    let line_aa = uv_deriv.max(0.000001.splat()) * 1.5.splat();

    let grid_uv = (uv_scaled.sfract() * 2.0 - 1.0).abs();

    let grid_smooth = grid_uv.smoothstep_each(line_width - line_aa..line_width + line_aa);

    let grid = 1.0 - grid_smooth.x * grid_smooth.y;

    let base_color = sm::vec!(0.0, 0.0, 0.0);
    let line_color = sm::vec!(1.0, 0.0, 1.0);

    let frag_color = grid.splat::<x3>().lerp_each(base_color, line_color);

    frag.attachments
        .color_iter()
        .next::<SurfaceFormat>()
        .set(frag_color.extend(1.0));
    // .blend(sm::Blend::add(), frag_color.extend(1.0));

    encoder.finish()
}

pub fn p2() -> Result<sm::results::RenderPipeline, sm::EncodingErrors> {
    let mut encoder = sm::start_encoding(sm::Settings::default())?;

    let mut drawcall = encoder.new_render_pipeline(sm::Indexing::Incremental);

    let colors = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)].to_gpu();

    let index = drawcall.vertices.index;
    let vert_color = colors.at(index);

    let time: f32x1 = drawcall.push_constants.get();
    let angle_offset = drawcall.vertices.instance_index.to_f32() * TAU / 5.0;

    let equilateral_triangle = [0, 1, 2]
        .map(|corner| (corner as f32 / 3.0) * TAU)
        .to_gpu()
        .map(move |a| a + time + angle_offset) // rotate via `time` push constant
        .map(|a| sm::vec!(a.cos(), a.sin()));

    let vert_pos = equilateral_triangle.at(index);

    let frag = drawcall
        .vertices
        .assemble(vert_pos * 0.7, sm::Draw::triangle_list(sm::Winding::Ccw))
        .rasterize(sm::Accuracy::default());

    let frag_color = frag.fill(vert_color);

    frag.attachments
        .color_iter()
        .next::<SurfaceFormat>()
        .blend(sm::Blend::add(), frag_color.extend(1.0));

    encoder.finish()
}
