struct Vertex {
  // We group vertices into "shapes", where a shape overall has the same color, and different parts
  // of the shape can have differing, dynamically-calculated offsets. Because many vertices in a
  // given shape will share the same parameters, we have the vertices index into an array containing
  // those parameters instead of storing inline.
  @location(0) base_position: vec2f,
  @location(1) color_index: u32,
  @location(2) offset_index: u32,
}

// The groups of colors
@group(0) @binding(1) var<storage, read> colors: array<vec4f>;
// The groups of dynamic offsets
@group(0) @binding(2) var<storage, read> offsets: array<vec2f>;

struct VSOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
}

@vertex fn vs(
  vertex: Vertex
) -> VSOutput {
  let position = vertex.base_position + offsets[vertex.offset_index];
  var vsOut: VSOutput;
  vsOut.position = vec4f(
    position * uni.scale + uni.offset, 0.0, 1.0
  );
  vsOut.color = colors[vertex.color_index];
  return vsOut;
}

// The overall 2D camera transform + bounding boxes. Only expected to change on screen resize.
struct Uniforms {
  scale: vec2f,
  offset: vec2f,
  lower_bound: vec2f,
  upper_bound: vec2f,
}
@group(0) @binding(0) var<uniform> uni: Uniforms;

@fragment fn fs(fsInput: VSOutput) -> @location(0) vec4f {
    let xy = fsInput.position.xy;
    if (all(uni.lower_bound <= xy) && all(xy <= uni.upper_bound)) {
        return fsInput.color;
    }
    discard;
}
