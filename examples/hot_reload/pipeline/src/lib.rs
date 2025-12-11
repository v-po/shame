use std::f32::consts::PI;
use std::f32::consts::TAU;

use shame::vec;
// use shame::GpuLayout;
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
    // p1()
    // p2()
    p4()
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

pub fn p4() -> Result<sm::results::RenderPipeline, sm::EncodingErrors> {
    let mut encoder = sm::start_encoding(sm::Settings::default())?;
    let drawcall = encoder.new_render_pipeline(sm::Indexing::Incremental);

    #[derive(sm::GpuLayout)]
    struct PushConstants {
        time: f32x1,
        aspect: f32x1,
        rot_x: f32x1,
        rot_y: f32x1,
    }
    let pc: PushConstants = drawcall.push_constants.get();

    let id32 = drawcall.vertices.index.to_u32();
    let u = ((id32 << 1) & 2).to_f32();
    let v = (id32 & 2).to_f32();
    let uv = sm::vec!(u, v);
    let xy = uv * 2.0 - 1.0;
    let clip_pos = sm::vec!(xy.x, -xy.y, 0.0, 1.0);

    let frag = drawcall
        .vertices
        .assemble(clip_pos, sm::Draw::triangle_list(sm::Winding::Cw))
        .rasterize(sm::Accuracy::default());

    let screen_pos = frag.fill(xy);

    let uv_corrected = sm::vec!(screen_pos.x * pc.aspect, screen_pos.y);

    // Camera
    let radius = 4.0;
    let cam_pos = sm::vec!(
        radius * pc.rot_x.sin() * pc.rot_y.cos(),
        radius * pc.rot_y.sin(),
        radius * pc.rot_x.cos() * pc.rot_y.cos(),
    );
    let target = sm::vec!(0.0, 0.0, 0.0);

    let fwd = (target - cam_pos).normalize();
    let world_up = sm::vec!(0.0, 1.0, 0.0);
    let right = fwd.cross(world_up).normalize();
    let up = right.cross(fwd);

    let ortho_scale = 1.6;
    let rd = fwd;
    let ro = cam_pos + (right * uv_corrected.x + up * uv_corrected.y) * ortho_scale;

    let t = sm::Cell::new(0.0);
    let hit = sm::Cell::new(false.to_gpu());

    // Shape params
    let num_segments = 11.0;
    let major_radius = 1.1;
    let box_radius = 0.002;
    let box_dims = sm::vec!(0.25, 0.25, 0.25);

    let hash31 = |p: f32x3| {
        let dot = p.dot(sm::vec!(12.9898, 78.233, 151.7182));
        (dot.sin() * 43758.5453).dfloor()
    };

    // Scene logic
    let get_scene_data = move |p: f32x3| {
        let orbit_angle = pc.time * 0.1;
        let raw_angle = p.xy().atan2();
        let active_angle = raw_angle + orbit_angle;

        let segment_size = TAU / num_segments;
        let segment_id = (active_angle / segment_size).round_ties_even();
        let segment_center_angle = segment_id * segment_size;

        let r = p.xy().length();
        let q_x = r - major_radius;
        let q_y = (active_angle - segment_center_angle) * major_radius;
        let q_z = p.z;

        // Twist
        let total_twist = PI * 0.5;
        let twist_val = (segment_id / num_segments) * total_twist - orbit_angle * (total_twist / TAU);
        let (s_tw, c_tw) = (twist_val.sin(), twist_val.cos());

        let local_pos = sm::vec!(q_x * c_tw - q_z * s_tw, q_y, q_x * s_tw + q_z * c_tw);

        let d = local_pos.abs() - box_dims;
        let dist = d.max(sm::zero()).length() + d.y.max(d.z).max(d.x).min(0.0) - box_radius;

        (dist, local_pos, segment_id)
    };

    let bounding_radius = 1.5;

    // Ray-Sphere Intersection
    let t_closest = -ro.dot(rd);
    let p_closest = ro + rd * t_closest;
    let dist_sq_from_center = p_closest.dot(p_closest);
    let radius_sq = bounding_radius * bounding_radius;

    let hits_sphere = dist_sq_from_center.lt(radius_sq);

    let half_chord = (radius_sq - dist_sq_from_center).max(0.0).sqrt();
    let t_sphere_enter = t_closest - half_chord;

    let t_start = t_sphere_enter.max(0.0);

    t.set(t_start);

    hits_sphere.then(move || {
        sm::for_range(0..40, move |_| {
            let p = ro + rd * t.get();
            let (dist, _, _) = get_scene_data(p);
            dist.abs().lt(0.002).then(move || hit.set(true.to_gpu()));
            (!hit.get()).then(move || {
                let max_dist = bounding_radius * 2.0 + t_sphere_enter;
                let in_bounds = t.get().lt(max_dist);
                in_bounds.then(move || {
                    t.set(t.get() + dist.min(10.0));
                });
            });
        });
    });

    // Shading
    let final_color = sm::Cell::new(sm::vec!(0.0, 0.0, 0.0));

    hit.get().then(move || {
        let p = ro + rd * t.get();

        // Normal
        let eps = 0.002;
        let dx = sm::vec!(eps, 0.0, 0.0);
        let dy = sm::vec!(0.0, eps, 0.0);
        let dz = sm::vec!(0.0, 0.0, eps);
        let nx = get_scene_data(p + dx).0 - get_scene_data(p - dx).0;
        let ny = get_scene_data(p + dy).0 - get_scene_data(p - dy).0;
        let nz = get_scene_data(p + dz).0 - get_scene_data(p - dz).0;
        let normal = sm::vec!(nx, ny, nz).normalize();

        let (_, local_pos, seg_id) = get_scene_data(p);

        // Wireframe
        let d = local_pos.abs() - box_dims;

        let edge_mask = d.smoothstep_each(-0.002..0.002);

        let is_edge_xy = edge_mask.x * edge_mask.y;
        let is_edge_yz = edge_mask.y * edge_mask.z;
        let is_edge_zx = edge_mask.z * edge_mask.x;

        let wire_strength = (is_edge_xy + is_edge_yz + is_edge_zx).clamp01();

        // Stars
        let segment_arc_len = (TAU * major_radius) / num_segments;
        let target_density = 55.0;
        let cells_per_seg = (segment_arc_len * target_density).round_ties_even();
        let density_y = cells_per_seg / segment_arc_len;
        let density_xz = target_density;

        let segment_offset = seg_id * sm::one::<f32, x3>(); //sm::vec!(5.9568, 98.233, 37.728) * seg_id;
        let grid_pos = sm::vec!(
            local_pos.x * density_xz,
            local_pos.y * density_y,
            local_pos.z * density_xz
        ) + segment_offset;
        let current_id = grid_pos.floor();
        let current_uv = grid_pos.dfloor();

        let star_accumulation = sm::Cell::new(0.0);

        sm::for_range(-1..=1, move |i| {
            sm::for_range(-1..=1, move |j| {
                sm::for_range(-1..=1, move |k| {
                    let offset = sm::vec!(i.to_f32(), j.to_f32(), k.to_f32());
                    let neighbor_id = current_id + offset;

                    let h1 = hash31(neighbor_id);

                    h1.gt(0.6).then(move || {
                        let h2 = hash31(neighbor_id + sm::vec!(13.0, 17.0, 19.0));
                        let h3 = hash31(neighbor_id + sm::vec!(101.0, 202.0, 303.0));

                        let center_offset = sm::vec!(h1, h2, h3) - 0.5;
                        let center_pos = offset + 0.5 + center_offset;

                        let d = current_uv.distance(center_pos);
                        let size = h2 * 0.55;
                        let star_shape = (1.0 - d.smoothstep((size * 0.8)..size)).clamp01();

                        let speed = h3 * 3.0;
                        let phase = h1 * 40.0;
                        let blink = (pc.time * speed + phase).sin().max(0.0).powf(8.0.to_gpu());

                        let val = star_shape * blink;
                        star_accumulation.set(star_accumulation.get().max(val));
                    });
                });
            });
        });

        let star_val = star_accumulation.get();
        let star_col: vec<f32, x3> = sm::vec!(1.0, 0.9, 0.6) * star_val * 5.0;

        // Lighting
        let light_dir: vec<f32, x3> = sm::vec!(0.5, 0.8, -0.5).normalize();
        let view_dir = (ro - p).normalize();
        let reflect_dir = (-light_dir).reflect(normal);
        let spec = view_dir.dot(reflect_dir).max(0.0).powf(32.0.to_gpu());

        // Composite
        let body_color = sm::vec!(0.0, 0.0, 0.0);
        let wire_base = sm::vec!(1.0, 1.0, 1.0);
        let wire_light = wire_base * (spec * 2.0 + 0.05);

        let shape_color = wire_light.lerp_each(body_color, wire_strength.splat());

        final_color.set(shape_color + star_col);
    });

    let col = final_color.get().powf(1.0 / 2.2);

    frag.attachments
        .color_iter()
        .next::<SurfaceFormat>()
        .set(col.extend(1.0));

    encoder.finish()
}
