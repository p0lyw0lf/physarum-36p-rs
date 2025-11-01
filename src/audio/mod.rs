mod fft;
pub mod inspectable_source;
pub mod worker;

/// Number of samples in the buffer. Must be a power of 2.
pub const SAMPLES: usize = 2048;
