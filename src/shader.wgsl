
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) bg_color: vec3<f32>,

}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) bg_color: vec3<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.bg_color = model.bg_color;
    out.tex_coords = model.tex_coords;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0)@binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    var col = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    var p = tex[3];

    return vec4<f32>(p*tex[0] + (1.0 -p)*col[0], 
        p*tex[1] + (1.0 - p)*col[1], 
        p*tex[2] + (1.0 - p)*col[2],
        1.0);
}

 

 