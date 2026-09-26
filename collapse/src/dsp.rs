//! Realtime-safe wet path: fixed-size FIR convolution + drive/tone/mix.

use crate::atlas::DEFAULT_IR_LEN;

pub const MAX_IR: usize = DEFAULT_IR_LEN;

pub struct CollapseDsp {
    /// Ring buffer of dry input (mono sum) for convolution.
    history: Vec<f32>,
    hist_pos: usize,
    /// Active impulse response (zero-padded to MAX_IR).
    ir: Vec<f32>,
    ir_len: usize,
    /// One-pole tone filter state.
    tone_lp: f32,
    sample_rate: f32,
}

impl Default for CollapseDsp {
    fn default() -> Self {
        Self {
            history: vec![0.0; MAX_IR],
            hist_pos: 0,
            ir: vec![0.0; MAX_IR],
            ir_len: 1,
            tone_lp: 0.0,
            sample_rate: 44_100.0,
        }
    }
}

impl CollapseDsp {
    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr.max(1.0);
    }

    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.hist_pos = 0;
        self.tone_lp = 0.0;
    }

    /// Hot-swap IR from a background Collapse. Not called from the audio thread
    /// until the plugin copies under a try_lock / swap protocol.
    pub fn set_ir(&mut self, samples: &[f32]) {
        self.ir.fill(0.0);
        let n = samples.len().min(MAX_IR).max(1);
        self.ir[..n].copy_from_slice(&samples[..n]);
        // Soft normalize
        let peak = self.ir[..n]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max)
            .max(1e-6);
        for s in &mut self.ir[..n] {
            *s /= peak;
        }
        self.ir_len = n;
        self.reset();
    }

    pub fn has_ir(&self) -> bool {
        self.ir_len > 1 || self.ir[0].abs() > 1e-6
    }

    #[inline]
    fn convolve_tick(&mut self, x: f32) -> f32 {
        self.history[self.hist_pos] = x;
        let mut acc = 0.0;
        let mut idx = self.hist_pos;
        let n = self.ir_len;
        for i in 0..n {
            acc += self.history[idx] * self.ir[i];
            if idx == 0 {
                idx = MAX_IR - 1;
            } else {
                idx -= 1;
            }
        }
        self.hist_pos += 1;
        if self.hist_pos >= MAX_IR {
            self.hist_pos = 0;
        }
        acc
    }

    #[inline]
    fn saturate(x: f32, drive: f32) -> f32 {
        let g = 1.0 + drive * 8.0;
        (x * g).tanh()
    }

    #[inline]
    fn tone_filter(&mut self, x: f32, tone: f32) -> f32 {
        // tone 0 = dark, 1 = bright
        let cutoff = 400.0 + tone.clamp(0.0, 1.0) * 10_000.0;
        let rc = 1.0 / (std::f32::consts::TAU * cutoff);
        let dt = 1.0 / self.sample_rate;
        let a = dt / (rc + dt);
        self.tone_lp += a * (x - self.tone_lp);
        // Blend lowpassed with raw for brightness
        self.tone_lp * (1.0 - tone * 0.65) + x * (tone * 0.65)
    }

    /// Process one stereo (or mono duplicated) frame. Returns (left, right).
    pub fn process_stereo(
        &mut self,
        left: f32,
        right: f32,
        mix: f32,
        drive: f32,
        tone: f32,
        output_gain: f32,
    ) -> (f32, f32) {
        let mono = 0.5 * (left + right);
        let wet_raw = if self.has_ir() {
            self.convolve_tick(mono)
        } else {
            // Passthrough grit so the plugin is audible before first Collapse
            mono
        };
        let wet = self.tone_filter(Self::saturate(wet_raw, drive), tone) * output_gain;
        let m = mix.clamp(0.0, 1.0);
        let dry_g = 1.0 - m;
        (left * dry_g + wet * m, right * dry_g + wet * m)
    }
}
