// Display shader that emulates CRT display.

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) scanline: vec2<f32>
}

@vertex fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var position = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0)
    );
    var tex_coord = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0)
    );

    var out: VertexOutput;
    out.pos = vec4<f32>(position[vertex_index], 0.0, 1.0);
    out.tex_coords = tex_coord[vertex_index];
    out.scanline.x = (out.pos.x + 1.0) * 320;
    out.scanline.y = (out.pos.y + 1.0) * 240;
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

fn adjust_color(in: f32) -> f32 {
    if in < 0.25 {
        return in * 2.0;
    } else {
        return 0.5 + (in - 0.25) * 0.666;
    }
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = i32(in.scanline.x);
    let scanline = i32(in.scanline.y);
    var darken = 1.0f;
    if (scanline % 2 == 1) {
        darken = 0.4;
    } else if (pixel % 2 == 1) {
        darken = 0.6;
    }
    var col = vec4<f32>(textureSample(tex, tex_sampler, in.tex_coords).rgb, 1.0);
    col.r = adjust_color(col.r);
    col.g = adjust_color(col.g);
    col.b = adjust_color(col.b);
    return col * darken;
}
