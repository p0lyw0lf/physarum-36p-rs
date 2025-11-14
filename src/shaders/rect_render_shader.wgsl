struct VertexShaderOutput {
    @builtin(position) position: vec4f,
    @location(0) texcoord: vec2f,
}

struct Uniforms {
  scale: vec2f,
  offset: vec2f,
  lower_bound: vec2f,
  upper_bound: vec2f,
}
@group(0) @binding(3) var<uniform> uni: Uniforms;

@vertex fn vs(
    @builtin(vertex_index) vertexIndex: u32,
) -> VertexShaderOutput {
    let pos = array(
        // 1st triangle
        vec2f(0.0, 0.0), // center
        vec2f(1.0, 0.0), // right, center
        vec2f(0.0, 1.0), // center, top

        // 2nd triangle
        vec2f(0.0, 1.0), // center, top
        vec2f(1.0, 0.0), // right, center
        vec2f(1.0, 1.0), // right, top
    );

    var vsOutput: VertexShaderOutput;
    let xy = pos[vertexIndex];
    let dims = textureDimensions(ourTexture);
    vsOutput.position = vec4f(xy * vec2f(dims) * uni.scale + uni.offset, 0.0, 1.0);
    vsOutput.texcoord = xy;
    return vsOutput;
}

@group(0) @binding(0) var ourSampler: sampler;
@group(0) @binding(1) var ourTexture: texture_2d<f32>;

@fragment fn fs(fsInput: VertexShaderOutput) -> @location(0) vec4f {
    let xy = fsInput.position.xy;
    if (all(uni.lower_bound <= xy) && all(xy <= uni.upper_bound)) {
        return textureSample(ourTexture, ourSampler, fsInput.texcoord);
    }
    discard;
}
