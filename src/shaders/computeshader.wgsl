// By Etienne Jacob, License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License
// Attribution to Sage Jenson's work explained in comments
// Ported from GLSL to WGSL by PolyWolf

struct Constants {
	width: u32,
	height: u32,
	reset_value: u32,
	deposit_factor: f32,
	decay_factor: f32,
}
@group(0) @binding(0) var<uniform> constants: Constants;

struct PointSettings {
	default_scaling_factor: f32,
	sd_base: f32,
	sd_exponent: f32,
	sd_amplitude: f32,
	sa_base: f32,
	sa_exponent: f32,
	sa_amplitude: f32,
	ra_base: f32,
	ra_exponent: f32,
	ra_amplitude: f32,
	md_base: f32,
	md_exponent: f32,
	md_amplitude: f32,
	sensor_bias_1: f32,
	sensor_bias_2: f32,
};
@group(0) @binding(1) var<uniform> params: PointSettings;

@group(1) @binding(0) var<storage, read> trailRead: texture_2d<f32>;
@group(1) @binding(1) var<storage, write> trailWrite: texture_2d<f32>;

@group(2) @binding(0) var<storage, read> particleParams: array<u32>;
@group(2) @binding(1) var<storage, read_write> particleCounters: array<u32>;
@group(2) @binding(2) var<storage, write> fbo_display: texture_2d<f32>;

const PI: f32 = radians(180.0);
const LIMIT: f32 = 100.0;

// In practice, this shader is only used to reset the counts (of particles) to zero for all pixels (at each iteration)

@compute @workgroup_size(32, 32, 1)
fn cs_setter(
	@builtin(global_invocation_id) id: vec3<u32>
) {
	particleCounters[id.x * params.height + id.y] = params.value;
}

///////////////////////////////////////////////////
// Randomness utils obtained from chatgpt

// A small, stand-alone 32-bit hashing function.
// Maps a single uint 'v' into another "scrambled" uint.
fn pcg_hash(v: u32) -> u32 {
	// These constants come from PCG-like mixing functions
	v = v * 747796405u + 2891336453u;
	let word = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
	return (word >> 22u) ^ word;
}
fn randFloat(state: ptr<function, u32>) -> f32 {
	// Hash and update 'state' each time we want a new random
	*state = pcg_hash(*state);
	// Convert to float in [0..1)
	//  4294967296 = 2^32
	return f32(state) * (1.0 / 4294967296.0);
}
fn randomPosFromParticle(particlePos: vec2f) -> vec2f {
	// Convert (x,y) to integer coordinates
	let ipos = vec2i(floor(particlePos));

	// Pack x in the low 16 bits, y in the high 16 bits
	// (Works if width, height <= 65535)
	let seed = (u32(ipos.x) & 0xFFFFu) | ((u32(ipos.y) & 0xFFFFu) << 16);

	// Generate two random floats in [0..1)
	let rx = randFloat(seed);
	let ry = randFloat(seed);

	// Scale them to [0..width] and [0..height] respectively
	return vec2f(rx * width, ry * height);
}
fn random01FromParticle(particlePos: vec2f) -> f32 {
	let ipos = vec2i(floor(particlePos));
	let seed = (u32(ipos.x) & 0xFFFFu) | ((u32(ipos.y) & 0xFFFFu) << 16);
	return randFloat(seed);
}
// End of randomness utils
///////////////////////////////////////////////////

fn getGridValue(pos: vec2f) -> f32 {
	return textureLoad(trailRead, vec2f(
		modf(pos.x + 0.5 + f32(width), f32(width)).fract,
		modf(pos.y + 0.5 + float(height), float(height)).fract,
	), 0).x;
}

fn senseFromAngle(angle: f32, pos: vec2f, heading: f32, sense_offset: f32) -> f32 {
	return getGridValue(vec2f(
		pos.x + sense_offset * cos(heading + angle),
		pos.y + sense_offset * sin(heading + angle),
	));
}

// This is the main shader.
// It updates the current particle's attributes (mostly position and heading).
// It also increases a counter on the pixel of the particle's new position, which will be used to add deposit in the deposit shader.
// Counter increased with atomicAdd function to be able to do it with many particles in parallel.

@compute @workgroup_size(128, 1, 1)
fn cs_move(
	@builtin(global_invocation_id) id: vec3<u32>
) {
	let particlePos = unpack2x16unorm(particlesArray[2 * id.x] * vec2f(width, height));
	let curProgressAndHeading = unpack2x16unorm(particlesArray[2 * id.x + 1] * vec2f(1.0, 2.0 * PI));

	let heading = curProgressAndHeading.y;
	let direction = vec2(cos(heading), sin(heading));

	///////////////////////////////////////////////////////////////////////////////////
	// Techniques/formulas from Sage Jenson (mxsage)
	// Sensing a value at particle pos or next to it...
	let currentSensedValue = getGridValue(particlePos + params.sensor_bias_2 * direction + vec2(0., params.sensor_bias_1));
	currentSensedValue *= params.default_scaling_factor;
	currentSensedValue = clamp(currentSensedValue, 0.000000001, 1.0);
	// For a current sensed value S,
	// physarum param = A + B * (S ^ C)
	// These A,B,C parameters are part of the data of a "Point"
	let sensorDistance = params.sd_base + params.sd_amplitude * pow(currentSensedValue, params.sd_exponent) * 250.0;
	let moveDistance = params.md_base + params.md_amplitude * pow(currentSensedValue, params.md_exponent) * 250.0;
	let sensorAngle = params.sa_base + params.sa_amplitude * pow(currentSensedValue, params.sa_exponent);
	let rotationAngle = params.ra_base + params.ra_amplitude * pow(currentSensedValue, params.ra_exponent);
	// 3 * 4 = 12 parameters + 2 with sensor bias
	///////////////////////////////////////////////////////////////////////////////////

	// sensing at 3 positions, as in the classic physarum algorithm
	let sensedLeft = senseFromAngle(-sensorAngle, particlePos, heading, sensorDistance);
	let sensedMiddle = senseFromAngle(0, particlePos, heading, sensorDistance);
	let sensedRight = senseFromAngle(sensorAngle, particlePos, heading, sensorDistance);

	var newHeading = heading;
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
	let px = particlePos.x + moveDistance * cos(newHeading);
	let py = particlePos.y + moveDistance * sin(newHeading);

	// position loop to keep pixel positions of the simulation canvas
	vec2 nextPos = vec2f(
		modf(px + float(width), float(width)).fract,
		modf(py + float(height), float(height)).fract,
	);

	let depositAmount: u32 = 1; // all particles add 1 on pixel count, could be more complex one day maybe
	// atomicAdd for increasing counter at pixel, in parallel computation
	atomicAdd(particleCounters[int(round(nextPos.x)) * height + int(round(nextPos.y))], depositAmount);

	///////////////////////////////////////////////////////////////////////////////////
	// Technique/formula from Sage Jenson (mxsage)
	// particles are regularly respawning, their progression is stored in particle data
	const reinitSegment = 0.0010; // respawn every 1/reinitSegment iterations
	let curProgress = curProgressAndHeading.x;
	if(curProgress < reinitSegment) {
		nextPos = randomPosFromParticle(particlePos);
	}
	let nextA = fract(curProgress + reinitSegment);
	///////////////////////////////////////////////////////////////////////////////////

	let nextPosUV = modf(nextPos, vec2(width, height)).fract / vec2(width, height);
	let newHeadingNorm = modf(newHeading, 2.0 * PI).fract / (2.0 * PI);
	let nextAandHeading = vec2f(nextA, fract(newHeadingNorm));

	// update particle data
	particlesArray[2 * id.x] = pack2x16unorm(nextPosUV);
	particlesArray[2 * id.x + 1] = pack2x16unorm(nextAandHeading);
}

/////////////////////////////////////////////////////
// This shader is looking at a single pixel.
// It adds deposit to the trail map from the number of particles on this pixel.
// It also sets the color of the pixel in the displayed image.

@compute @workgroup_size(32, 32, 1)
fn cs_deposit(
	@builtin(global_invocation_id) id: vec3<u32>,
) {
	let pix = vec2i(id.xy);

	let prevColor = textureLoad(trailRead, pix, 0).xy; // Getting the trail map color on current pixel

	let count = f32(particleCounters[id.x * height + id.y]); // number of particles on the pixel

	// The following 3 lines of code are Bleuje's own innovation (looks like with the license, attribution is required if you use this :) ),
	// A way to define an amount of added trail in function of the number of particles on the pixel:
	let limitedCount = min(count, LIMIT);
	let addedDeposit = sqrt(limitedCount) * depositFactor;

	// Trail map update
	let val = prevColor.x + addedDeposit;
	textureStore(trailWrite, pix, vec4(val, 0.0, 0.0, 0.0));

	// Mapping the count on pixel to color intensity
	let countColorValue = tanh(pow(count / 10.0, 1.7));
	let col = clamp(vec3(countColorValue), vec3(0.0), vec3(1.0));
	let outputColor = vec4(col, 1.0);

	textureStore(fbo_display, pix, outputColor);
}

fn LoopedPosition(pos: vec2i) -> vec2i {
	return vec2i(
		mod(pos.x + width, width).frac,
		mod(pos.y + height, height).frac,
	);
}

/////////////////////////////////////////////////////
// Shader for trail map diffusion and decay

@compute @workgroup_size(32, 32, 1)
fn cs_diffusion(
	@builtin(global_invocation_id) id: vec3<u32>,
) {
	let pos = vec2i(id.xy);

	let colorSum = vec2f(0.0);
	let kernelSize = 1.0;
	for(var i = -kernelSize; i < kernelSize + 0.5; i += 1.0) {
		for(var j = -kernelSize; j < kernelSize + 0.5; j += 1.0) {
			colorSum += textureLoad(trailRead, LoopedPosition(pos - vec2i(i, j)), 0).xy;
		}
	}

	let c = colorSum / pow(2.0 * kernelSize + 1.0, 2.0);

	let decayed = c.x * decayFactor;
	let cOutput = vec4(decayed, 0.0, 0.0, 0.0);

	imageStore(trailWrite, ivec2(id.xy), cOutput);
}
