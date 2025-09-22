#version 440

uniform int width;
uniform int height;
uniform float decayFactor;

layout(r16f,binding=0) uniform readonly image2D trailRead;
layout(r16f,binding=1) uniform writeonly image2D trailWrite;

ivec2 LoopedPosition(ivec2 pos) {
	return ivec2(mod(pos.x + width, width), mod(pos.y + height, height));
}

// Shader for trail map diffusion and decay

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;
void main() {
	ivec2 pos = ivec2(gl_GlobalInvocationID.xy);

	vec2 colorSum = vec2(0.);

	float kernelSize = 1.0;

	for(float i = -kernelSize; i < kernelSize + 0.5; i += 1.0) {
		for(float j = -kernelSize; j < kernelSize + 0.5; j += 1.0) {
			colorSum += imageLoad(trailRead, LoopedPosition(pos - ivec2(i, j))).xy;
		}
	}

	vec2 c = colorSum / pow(2 * kernelSize + 1.0, 2.0);

	float decayed = c.x * decayFactor;
	vec4 cOutput = vec4(decayed, 0, 0, 0);

	imageStore(trailWrite, ivec2(gl_GlobalInvocationID.xy), cOutput);
}
