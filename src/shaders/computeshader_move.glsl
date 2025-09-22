#version 440

// By Etienne Jacob, License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License
// Attribution to Sage Jenson's work explained in comments

#define PI 3.141592

uniform int width;
uniform int height;

struct PointSettings {
	float defaultScalingFactor;
	float SensorDistance0;
	float SD_exponent;
	float SD_amplitude;
	float SensorAngle0;
	float SA_exponent;
	float SA_amplitude;
	float RotationAngle0;
	float RA_exponent;
	float RA_amplitude;
	float MoveDistance0;
	float MD_exponent;
	float MD_amplitude;
	float SensorBias1;
	float SensorBias2;
};

layout(std430,binding=5) buffer parameters
{
	PointSettings pointParams[];
};

layout(r16f,binding=0) uniform readonly image2D trailRead;
layout(std430,binding=3) buffer mutex
{
	uint particlesCounter[];
};

layout(std430, binding=2) buffer particle{
	uint particlesArray[];
};

///////////////////////////////////////////////////
// Randomness utils obtained from chatgpt

// A small, stand-alone 32-bit hashing function.
// Maps a single uint 'v' into another "scrambled" uint.
uint pcg_hash(uint v) {
	// These constants come from PCG-like mixing functions
	v = v * 747796405u + 2891336453u;
	uint word = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
	return (word >> 22u) ^ word;
}
float randFloat(inout uint state) {
	// Hash and update 'state' each time we want a new random
	state = pcg_hash(state);
	// Convert to float in [0..1)
	//  4294967296 = 2^32
	return float(state) * (1.0 / 4294967296.0);
}
vec2 randomPosFromParticle(in vec2 particlePos) {
	// Convert (x,y) to integer coordinates
	ivec2 ipos = ivec2(floor(particlePos));

	// Pack x in the low 16 bits, y in the high 16 bits
	// (Works if width, height <= 65535)
	uint seed = (uint(ipos.x) & 0xFFFFu) | ((uint(ipos.y) & 0xFFFFu) << 16);

	// Generate two random floats in [0..1)
	float rx = randFloat(seed);
	float ry = randFloat(seed);

	// Scale them to [0..width] and [0..height] respectively
	return vec2(rx * width, ry * height);
}
float random01FromParticle(in vec2 particlePos) {
	ivec2 ipos = ivec2(floor(particlePos));
	uint seed = (uint(ipos.x) & 0xFFFFu) | ((uint(ipos.y) & 0xFFFFu) << 16);
	return randFloat(seed);
}
// End of randomness utils
///////////////////////////////////////////////////

float getGridValue(vec2 pos) {
	return imageLoad(trailRead, ivec2(mod(pos.x + 0.5 + float(width), float(width)), mod(pos.y + 0.5 + float(height), float(height)))).x;
}

float senseFromAngle(float angle, vec2 pos, float heading, float so) {
	return getGridValue(vec2(pos.x + so * cos(heading + angle), pos.y + so * sin(heading + angle)));
}

// This is the main shader.
// It updates the current particle's attributes (mostly position and heading).
// It also increases a counter on the pixel of the particle's new position, which will be used to add deposit in the deposit shader.
// Counter increased with atomicAdd function to be able to do it with many particles in parallel.

layout(local_size_x = 128, local_size_y = 1, local_size_z = 1) in;
void main() {

	PointSettings params = pointParams[0]; // "Pen Point" parameters

	vec2 particlePos = unpackUnorm2x16(particlesArray[2 * gl_GlobalInvocationID.x]) * vec2(width, height);
	vec2 curProgressAndHeading = unpackUnorm2x16(particlesArray[2 * gl_GlobalInvocationID.x + 1]) * vec2(1.0, 2.0 * PI);

	float heading = curProgressAndHeading.y;
	vec2 direction = vec2(cos(heading), sin(heading));

	///////////////////////////////////////////////////////////////////////////////////
	// Techniques/formulas from Sage Jenson (mxsage)
	// Sensing a value at particle pos or next to it...
	float SensorBias1 = params.SensorBias1;
	float SensorBias2 = params.SensorBias2;
	float currentSensedValue = getGridValue(particlePos + SensorBias2 * direction + vec2(0., SensorBias1));
	currentSensedValue *= params.defaultScalingFactor;
	currentSensedValue = clamp(currentSensedValue, 0.000000001, 1.0);
	// For a current sensed value S,
	// physarum param = A + B * (S ^ C)
	// These A,B,C parameters are part of the data of a "Point"
	float sensorDistance = params.SensorDistance0 + params.SD_amplitude * pow(currentSensedValue, params.SD_exponent) * 250.0;
	float moveDistance = params.MoveDistance0 + params.MD_amplitude * pow(currentSensedValue, params.MD_exponent) * 250.0;
	float sensorAngle = params.SensorAngle0 + params.SA_amplitude * pow(currentSensedValue, params.SA_exponent);
	float rotationAngle = params.RotationAngle0 + params.RA_amplitude * pow(currentSensedValue, params.RA_exponent);
	// 3 * 4 = 12 parameters + 2 with sensor bias
	///////////////////////////////////////////////////////////////////////////////////

	// sensing at 3 positions, as in the classic physarum algorithm
	float sensedLeft = senseFromAngle(-sensorAngle, particlePos, heading, sensorDistance);
	float sensedMiddle = senseFromAngle(0, particlePos, heading, sensorDistance);
	float sensedRight = senseFromAngle(sensorAngle, particlePos, heading, sensorDistance);

	float newHeading = heading;
	// heading update, as in the classic physarum algorithm
	if(sensedMiddle > sensedLeft && sensedMiddle > sensedRight) {
		;
	} else if(sensedMiddle < sensedLeft && sensedMiddle < sensedRight) {
		newHeading = (random01FromParticle(particlePos) < 0.5 ? heading - rotationAngle : heading + rotationAngle);
	} else if(sensedRight < sensedLeft) {
		newHeading = heading - rotationAngle;
	} else if(sensedLeft < sensedRight) {
		newHeading = heading + rotationAngle;
	}

	// position update of the classic physarum algorithm
	float px = particlePos.x + moveDistance * cos(newHeading);
	float py = particlePos.y + moveDistance * sin(newHeading);

	// position loop to keep pixel positions of the simulation canvas
	vec2 nextPos = vec2(mod(px + float(width), float(width)), mod(py + float(height), float(height)));

	uint depositAmount = uint(1); // all particles add 1 on pixel count, could be more complex one day maybe
	// atomicAdd for increasing counter at pixel, in parallel computation
	atomicAdd(particlesCounter[int(round(nextPos.x)) * height + int(round(nextPos.y))], depositAmount);

	///////////////////////////////////////////////////////////////////////////////////
	// Technique/formula from Sage Jenson (mxsage)
	// particles are regularly respawning, their progression is stored in particle data
	const float reinitSegment = 0.0010; // respawn every 1/reinitSegment iterations
	float curProgress = curProgressAndHeading.x;
	if(curProgress < reinitSegment) {
		nextPos = randomPosFromParticle(particlePos);
	}
	float nextA = fract(curProgress + reinitSegment);
	///////////////////////////////////////////////////////////////////////////////////

	vec2 nextPosUV = mod(nextPos, vec2(width, height)) / vec2(width, height);
	float newHeadingNorm = mod(newHeading, 2.0 * PI) / (2.0 * PI);
	vec2 nextAandHeading = vec2(nextA, fract(newHeadingNorm));

	// update particle data
	particlesArray[2 * gl_GlobalInvocationID.x] = packUnorm2x16(nextPosUV);
	particlesArray[2 * gl_GlobalInvocationID.x + 1] = packUnorm2x16(nextAandHeading);
}
