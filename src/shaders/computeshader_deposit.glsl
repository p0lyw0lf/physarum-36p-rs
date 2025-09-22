#version 440

uniform int width;
uniform int height;
uniform float depositFactor;

layout(std430,binding=3) buffer mutex
{
	uint particlesCounter[];
};

layout(r16f,binding=0) uniform readonly image2D trailRead;
layout(r16f,binding=1) uniform writeonly image2D trailWrite;
layout(rgba8,binding=4) uniform writeonly image2D displayWrite;

/////////////////////////////////////////////////////
// This shader is looking at a single pixel.
// It adds deposit to the trail map from the number of particles on this pixel.
// It also sets the color of the pixel in the displayed image.

layout(local_size_x = 32, local_size_y = 32, local_size_z = 1) in;
void main() {
	ivec2 pix = ivec2(gl_GlobalInvocationID.xy);

	vec2 prevColor = imageLoad(trailRead, ivec2(gl_GlobalInvocationID.xy)).xy; // Getting the trail map color on current pixel

	float count = float(particlesCounter[gl_GlobalInvocationID.x * height + gl_GlobalInvocationID.y]); // number of particles on the pixel

	// The following 3 lines of code are my own innovation (looks like with the license, attribution is required if you use this :) ),
	// a way to define an amount of added trail in function of the number of particles on the pixel
	const float LIMIT = 100.0;
	float limitedCount = min(count, LIMIT);
	float addedDeposit = sqrt(limitedCount) * depositFactor;

	// Trail map update
	float val = prevColor.x + addedDeposit;
	imageStore(trailWrite, ivec2(gl_GlobalInvocationID.xy), vec4(val, 0., 0, 0));

	// Mapping the count on pixel to color intensity
	float countColorValue = tanh(pow(count / 10.0, 1.7));
	vec3 col = clamp(vec3(countColorValue), vec3(0.), vec3(1.));
	vec4 outputColor = vec4(col, 1.0);

	imageStore(displayWrite, ivec2(gl_GlobalInvocationID.xy), outputColor);
}
