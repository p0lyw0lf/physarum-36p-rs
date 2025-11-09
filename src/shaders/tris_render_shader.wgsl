struct StaticVertex {
  // base_position is specified separately from offset because it is expected that:
  // 1. base_position is expensive to calculate and will not change all that often
  // 2. offset is cheap to calculate and changes every frame
  @location(1) color: vec4f,
  @location(0) base_position: vec4f, // zw indices are purely for padding
}

struct DynamicVertex {
  // TODO: do I want to express this as a matrix instsead? Would likely need to do stuff w/ indexing
  // in that case, it's much more expensive to store mat3x3f per-vertex than it is a single extra
  // vec2f lol
  @location(2) offset: vec2f,
}

struct VSOutput {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
}

@vertex fn vs(
  svert: StaticVertex,
  dvert: DynamicVertex,
) -> VSOutput {
  var vsOut: VSOutput;
  vsOut.position = vec4f(
    (svert.base_position.xy + dvert.offset) * uni.scale + uni.offset, 0.0, 1.0
  );
  vsOut.color = svert.color;
  return vsOut;
}

// The overall 2D camera transform + bounding boxes
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
