//! Audio equalizer using biquad peaking EQ filters.
//!
//! 7-band parametric EQ with per-band gain control (-12dB to +12dB).
//! Uses the Audio EQ Cookbook (Robert Bristow-Johnson) formulas.

use std::sync::atomic::{AtomicI32, Ordering};

/// Number of equalizer bands.
pub const NUM_BANDS: usize = 7;

/// Band center frequencies (Hz).
pub const BAND_FREQS: [f32; NUM_BANDS] = [60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0];

/// Global EQ gains stored as i32 (gain_db * 100 for precision).
/// Range: -1200 to +1200 (i.e. -12.0 dB to +12.0 dB).
static EQ_GAINS: [AtomicI32; NUM_BANDS] = [
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
];

/// Whether the EQ is enabled.
static EQ_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Set gain for a specific band (in dB, clamped to ±12).
pub fn set_gain(band: usize, db: f32) {
    if band < NUM_BANDS {
        let clamped = db.clamp(-12.0, 12.0);
        EQ_GAINS[band].store((clamped * 100.0) as i32, Ordering::Relaxed);
    }
}

/// Get all band gains as an array.
pub fn get_all_gains() -> [f32; NUM_BANDS] {
    let mut gains = [0.0f32; NUM_BANDS];
    for i in 0..NUM_BANDS {
        gains[i] = EQ_GAINS[i].load(Ordering::Relaxed) as f32 / 100.0;
    }
    gains
}

/// Set all band gains at once.
pub fn set_all_gains(gains: &[f32]) {
    for (i, &g) in gains.iter().enumerate().take(NUM_BANDS) {
        set_gain(i, g);
    }
}

/// Enable/disable the equalizer.
pub fn set_enabled(enabled: bool) {
    EQ_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Check if the equalizer is enabled.
pub fn is_enabled() -> bool {
    EQ_ENABLED.load(Ordering::Relaxed)
}

/// Second-order biquad filter (Direct Form II Transposed).
struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl BiquadFilter {
    /// Create a peaking EQ filter.
    /// - `freq`: center frequency (Hz)
    /// - `gain_db`: gain in dB (positive = boost, negative = cut)
    /// - `q`: quality factor (bandwidth)
    /// - `sample_rate`: sample rate (Hz)
    fn peaking_eq(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha / a;

        // Normalize by a0
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Process a single sample (Direct Form II Transposed).
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        output
    }

    /// Update filter coefficients without resetting state.
    fn update(&mut self, freq: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let new = Self::peaking_eq(freq, gain_db, q, sample_rate);
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
        // Keep z1, z2 state for smooth transition
    }
}

/// Multi-band equalizer with per-channel filter chains.
pub struct Equalizer {
    /// filters[channel][band]
    filters: Vec<Vec<BiquadFilter>>,
    sample_rate: f32,
    channels: usize,
    /// Cached gains to detect when we need to recalculate coefficients
    cached_gains: [f32; NUM_BANDS],
}

impl Equalizer {
    /// Create a new equalizer for the given sample rate and channel count.
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let sr = sample_rate as f32;
        let ch = channels as usize;
        let gains = get_all_gains();

        let filters: Vec<Vec<BiquadFilter>> = (0..ch)
            .map(|_| {
                BAND_FREQS
                    .iter()
                    .enumerate()
                    .map(|(i, &freq)| BiquadFilter::peaking_eq(freq, gains[i], 1.4, sr))
                    .collect()
            })
            .collect();

        Self {
            filters,
            sample_rate: sr,
            channels: ch,
            cached_gains: gains,
        }
    }

    /// Process interleaved samples in-place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !is_enabled() {
            return;
        }

        // Check if gains changed and update coefficients
        let current_gains = get_all_gains();
        if current_gains != self.cached_gains {
            for ch in 0..self.channels {
                for (band, &freq) in BAND_FREQS.iter().enumerate() {
                    self.filters[ch][band].update(freq, current_gains[band], 1.4, self.sample_rate);
                }
            }
            self.cached_gains = current_gains;
        }

        // Check if all gains are zero (bypass)
        let all_flat = current_gains.iter().all(|&g| g.abs() < 0.01);
        if all_flat {
            return;
        }

        // Apply filters to each sample
        for frame in samples.chunks_mut(self.channels) {
            for (ch_idx, sample) in frame.iter_mut().enumerate() {
                if ch_idx < self.channels {
                    for band in 0..NUM_BANDS {
                        *sample = self.filters[ch_idx][band].process(*sample);
                    }
                }
            }
        }
    }
}
