use bytemuck::NoUninit;

pub const SIMULATION_WIDTH: u32 = 1280;
pub const SIMULATION_HEIGHT: u32 = 736;
pub const SIMULATION_WORK_GROUP_SIZE: u32 = 32;
pub const SIMULATION_NUM_PARTICLES: usize = 512 * 512 * 22;

/// MUST exactly match the definition in computeshader.wgsl
#[repr(C)]
#[derive(NoUninit, Copy, Clone)]
pub struct Constants {
    pub width: u32,
    pub height: u32,
    pub reset_value: u32,
    pub deposit_factor: f32,
    pub decay_factor: f32,
}

/// MUST exactly match the definition in computeshader.wgsl
#[repr(C)]
#[derive(NoUninit, Copy, Clone)]
pub struct PointSettings {
    pub default_scaling_factor: f32,
    pub sd_base: f32,
    pub sd_exponent: f32,
    pub sd_amplitude: f32,
    pub sa_base: f32,
    pub sa_exponent: f32,
    pub sa_amplitude: f32,
    pub ra_base: f32,
    pub ra_exponent: f32,
    pub ra_amplitude: f32,
    pub md_base: f32,
    pub md_exponent: f32,
    pub md_amplitude: f32,
    pub sensor_bias_1: f32,
    pub sensor_bias_2: f32,
}

pub const CONSTANTS: Constants = Constants {
    width: SIMULATION_WIDTH,
    height: SIMULATION_HEIGHT,
    reset_value: 0,
    deposit_factor: 0.003,
    decay_factor: 0.75,
};
