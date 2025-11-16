use rodio::{Sample, SampleRate};

use super::SAMPLES;

struct FrequencyRange {
    lo: f32,
    hi: f32,
}

/// Defined frequency ranges that we want to plot graphically. All defined in terms of Hz.
const FREQUENCY_RANGES: &[FrequencyRange] = {
    const fn fr(lo: f32, hi: f32) -> FrequencyRange {
        FrequencyRange { lo, hi }
    }

    const SUB_BASS: FrequencyRange = fr(20.0, 80.0);
    const BASS: FrequencyRange = fr(80.0, 250.0);
    const LOW_MIDS: FrequencyRange = fr(250.0, 500.0);
    const MIDS: FrequencyRange = fr(500.0, 2_000.0);
    const HIGH_MIDS: FrequencyRange = fr(2_000.0, 6_000.0);
    const HIGHS: FrequencyRange = fr(6_000.0, 10_000.0);

    &[SUB_BASS, BASS, LOW_MIDS, MIDS, HIGH_MIDS, HIGHS]
};
pub const NUM_FREQUENCY_RANGES: usize = FREQUENCY_RANGES.len();

/// Given a list of samples, compute the FFT & bucket the results into pre-determined frequency
/// ranges.
pub fn fft_buckets(samples: &mut [Sample; SAMPLES], sample_rate: SampleRate) -> Vec<f32> {
    let spectrum = microfft::real::rfft_2048(samples);
    // since the real-valued coefficient at the Nyquist frequency is packed into the
    // imaginary part of the DC bin, it must be cleared before computing the amplitudes
    spectrum[0].im = 0.0;

    let amplitudes: Vec<f32> = spectrum.iter().map(|c| c.norm_sqr().sqrt()).collect();
    // How much frequency does each bucket produce?
    let resolution = sample_rate as f32 / (SAMPLES / 2) as f32;

    FREQUENCY_RANGES
        .iter()
        .map(|r| {
            let index_lo = (r.lo / resolution).floor() as usize;
            let index_hi = (r.hi / resolution).ceil() as usize;
            // TODO: I got an index-out-of-range crash here somehow. Apparently the sample_rate
            // must have been off somehow?? (like way too low). Doing this for safety, doesn't
            // affect it when it's running normally.
            let index_lo = index_lo.clamp(0, amplitudes.len() - 1);
            let index_hi = index_hi.clamp(0, amplitudes.len() - 1);

            amplitudes[index_lo..index_hi].iter().sum::<f32>() / (index_hi - index_lo) as f32
        })
        .collect()
}
