//! Port of cavacore from cava (https://github.com/karlstav/cava),
//! Copyright (c) Karl Stavestrand, MIT License.
//!
//! Original MIT notice:
//!
//! Copyright (c) 2022 Karl Stavestrand <karl@stavestrand.no>
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in
//! all copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! DEALINGS IN THE SOFTWARE.
//!
//! Differences from upstream (master branch, cavacore.c):
//! - mono only (`audio_channels = 1`), `CAVA_SCALING_LINEAR` output;
//! - rustfft replaces fftw (upstream runs r2c plans; we run a complex FFT on
//!   real input and use the same N/2+1 spectrum bins);
//! - input samples are f32 in -1..1 and are scaled by 32767 so the EQ/sens
//!   constants behave like upstream's int16-scaled doubles;
//! - `bar_freqs` = geometric centre of each band's cut-off pair, for labels.

use std::f64::consts::PI;
use std::sync::Arc;

use rustfft::Fft;
use rustfft::num_complex::Complex64;

pub struct CavaCore {
    number_of_bars: usize,
    rate: u32,
    autosens: bool,
    noise_reduction: f64,

    sens: f64,
    sens_init: bool,
    /// Adaptive frame-rate estimate, like upstream (starts at 75 fps and
    /// converges to rate/new_samples per call).
    framerate: f64,
    frame_skip: u32,

    fft_size: usize,
    fft_bass_size: usize,
    input_buffer: Vec<f64>,

    bass_multiplier: Vec<f64>,
    multiplier: Vec<f64>,

    fft_bass: Arc<dyn Fft<f64>>,
    fft_mid: Arc<dyn Fft<f64>>,
    bass_raw: Vec<f64>,
    mid_raw: Vec<f64>,
    bass_spectrum: Vec<Complex64>,
    mid_spectrum: Vec<Complex64>,

    lower_cut_off: Vec<usize>,
    upper_cut_off: Vec<usize>,
    eq: Vec<f64>,
    /// Kept for upstream parity (used by cut_off_frequencies()).
    #[allow(dead_code)]
    cut_off_frequency: Vec<f32>,
    bar_freqs: Vec<f32>,
    bass_cut_off_bar: usize,

    cava_fall: Vec<f64>,
    cava_mem: Vec<f64>,
    cava_peak: Vec<f64>,
    prev_cava_out: Vec<f64>,

    out: Vec<f32>,
}

impl CavaCore {
    /// `noise_reduction` in 0..1 (cava default 0.77), `low/high_cut_off` in Hz
    /// (cava default 50 / 10000). `high_cut_off` must stay below rate/2.
    pub fn new(
        number_of_bars: usize,
        rate: u32,
        autosens: bool,
        noise_reduction: f32,
        low_cut_off: f32,
        high_cut_off: f32,
    ) -> Self {
        // FFT sizes exactly like cava_init.
        let mut fft_buffer_size = 512usize;
        if rate > 8125 && rate <= 16250 {
            fft_buffer_size *= 2;
        } else if rate > 16250 && rate <= 32500 {
            fft_buffer_size *= 4;
        } else if rate > 32500 && rate <= 75000 {
            fft_buffer_size *= 8;
        } else if rate > 75000 && rate <= 150000 {
            fft_buffer_size *= 16;
        } else if rate > 150000 && rate <= 300000 {
            fft_buffer_size *= 32;
        } else if rate > 300000 {
            fft_buffer_size *= 64;
        }

        let fft_bass_size = fft_buffer_size * 2;
        let fft_size = fft_buffer_size;
        let number_of_bars = number_of_bars.clamp(1, fft_size / 2 + 1);
        let high_cut_off = high_cut_off.min(rate as f32 / 2.0);

        let mut planner = rustfft::FftPlanner::new();
        let fft_bass = planner.plan_fft_forward(fft_bass_size);
        let fft_mid = planner.plan_fft_forward(fft_size);

        let bass_multiplier = (0..fft_bass_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (fft_bass_size - 1) as f64).cos()))
            .collect();
        let multiplier = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (fft_size - 1) as f64).cos()))
            .collect();

        // Cut-off frequency table (ported verbatim from cava_init).
        let upper_cut_off = high_cut_off as f64;
        let bass_cut_off = 100.0f32;

        let frequency_constant = (low_cut_off as f64 / upper_cut_off).log10()
            / (1.0 / (number_of_bars as f64 + 1.0) - 1.0);

        let mut relative_cut_off = vec![0f32; number_of_bars + 1];
        let mut cut_off_frequency = vec![0f32; number_of_bars + 1];
        let mut lower = vec![0usize; number_of_bars + 1];
        let mut upper = vec![0usize; number_of_bars + 1];

        let mut bass_cut_off_bar = 0usize;
        let mut first_bar = 1;

        let min_bandwidth = rate as f32 / fft_bass_size as f32;

        for n in 0..number_of_bars + 1 {
            let mut bar_distribution_coefficient = -frequency_constant;
            bar_distribution_coefficient +=
                (n as f64 + 1.0) / (number_of_bars as f64 + 1.0) * frequency_constant;
            cut_off_frequency[n] = (upper_cut_off * 10f64.powf(bar_distribution_coefficient)) as f32;

            if n > 0 && cut_off_frequency[n - 1] >= cut_off_frequency[n] {
                cut_off_frequency[n] = cut_off_frequency[n - 1] + min_bandwidth;
            }

            relative_cut_off[n] = cut_off_frequency[n] / (rate as f32 / 2.0);

            if cut_off_frequency[n] < bass_cut_off {
                // BASS (index into the bass FFT spectrum)
                lower[n] =
                    (relative_cut_off[n] * (fft_bass_size / 2) as f32) as usize;
                bass_cut_off_bar += 1;
                if bass_cut_off_bar > 1 {
                    first_bar = 0;
                }
                if lower[n] > fft_bass_size / 2 {
                    lower[n] = fft_bass_size / 2;
                }
            } else {
                // MID + TREBLE (index into the mid FFT spectrum)
                lower[n] = (relative_cut_off[n] * (fft_size / 2) as f32).ceil() as usize;
                if n == bass_cut_off_bar {
                    first_bar = 1;
                    if n > 0 {
                        upper[n - 1] = (relative_cut_off[n] * (fft_bass_size / 2) as f32)
                            .max(1.0) as usize
                            - 1;
                    }
                } else {
                    first_bar = 0;
                }
                if lower[n] > fft_size / 2 {
                    lower[n] = fft_size / 2;
                }
            }

            if n > 0 {
                if first_bar == 0 {
                    upper[n - 1] = lower[n].saturating_sub(1);
                    // Push the spectrum up if the exponential table gets
                    // "clumped" in the bass (at least one bin per bar).
                    if lower[n] <= lower[n - 1] {
                        let room_for_more = if n < bass_cut_off_bar {
                            lower[n - 1] + 1 < fft_bass_size / 2 + 1
                        } else {
                            lower[n - 1] + 1 < fft_size / 2 + 1
                        };
                        if room_for_more {
                            lower[n] = lower[n - 1] + 1;
                            upper[n - 1] = lower[n] - 1;
                        }
                    }
                } else if upper[n - 1] < lower[n - 1] {
                    upper[n - 1] = lower[n - 1] + 1;
                }
            }

            // Actual cut-off frequency after the integer-bin fix-ups.
            relative_cut_off[n] = if n < bass_cut_off_bar {
                lower[n] as f32 / (fft_bass_size / 2) as f32
            } else {
                lower[n] as f32 / (fft_size / 2) as f32
            };
            cut_off_frequency[n] = relative_cut_off[n] * (rate as f32 / 2.0);
        }

        // Hard-coded EQ (ported verbatim).
        let mut eq = vec![0f64; number_of_bars + 1];
        for n in 0..number_of_bars {
            eq[n] = 1.0 / 2f64.powi(28);
            eq[n] *= (cut_off_frequency[n + 1] as f64).powf(0.85);
            if n < bass_cut_off_bar {
                eq[n] /= (fft_bass_size as f64).log2();
            } else {
                eq[n] /= (fft_size as f64).log2();
            }
            eq[n] /= (upper[n] - lower[n] + 1) as f64;
        }

        let bar_freqs = (0..number_of_bars)
            .map(|n| (cut_off_frequency[n] * cut_off_frequency[n + 1]).sqrt())
            .collect();

        Self {
            number_of_bars,
            rate,
            autosens,
            noise_reduction: noise_reduction as f64,
            sens: 1.0,
            sens_init: true,
            framerate: 75.0,
            frame_skip: 1,
            fft_size,
            fft_bass_size,
            input_buffer: vec![0.0; fft_bass_size],
            bass_multiplier,
            multiplier,
            fft_bass,
            fft_mid,
            bass_raw: vec![0.0; fft_bass_size],
            mid_raw: vec![0.0; fft_size],
            bass_spectrum: vec![Complex64::ZERO; fft_bass_size],
            mid_spectrum: vec![Complex64::ZERO; fft_size],
            lower_cut_off: lower,
            upper_cut_off: upper,
            eq,
            cut_off_frequency,
            bar_freqs,
            bass_cut_off_bar,
            cava_fall: vec![0.0; number_of_bars],
            cava_mem: vec![0.0; number_of_bars],
            cava_peak: vec![0.0; number_of_bars],
            prev_cava_out: vec![0.0; number_of_bars],
            out: vec![0.0; number_of_bars],
        }
    }

    /// Samples held by the input ring buffer; used to rewind on seeks.
    pub fn input_buffer_size(&self) -> usize {
        self.input_buffer.len()
    }

    /// Geometric centre frequency of each bar.
    pub fn bar_freqs(&self) -> &[f32] {
        &self.bar_freqs
    }

    /// Cut-off edges (n+1 values), kept for parity with upstream.
    #[allow(dead_code)]
    /// Current auto-sensitivity gain (diagnostics).
    pub fn sens(&self) -> f64 {
        self.sens
    }

    /// Current adaptive frame-rate estimate (diagnostics).
    pub fn framerate(&self) -> f64 {
        self.framerate
    }

    /// Cut-off edges (n+1 values), kept for parity with upstream.
    #[allow(dead_code)]
    pub fn cut_off_frequencies(&self) -> &[f32] {
        &self.cut_off_frequency
    }

    /// Feed new mono f32 samples (-1..1, any count, may be 0) and recompute
    /// the bars. Returns bar heights in 0..1.
    pub fn execute(&mut self, new_samples: &[f32]) -> &[f32] {
        let new = new_samples.len().min(self.input_buffer.len());
        let mut silence = true;

        if new > 0 {
            // Adaptive framerate (upstream formula).
            self.framerate -= self.framerate / 64.0;
            self.framerate += (self.rate * self.frame_skip) as f64 / new as f64 / 64.0;
            self.frame_skip = 1;

            // Shift the input buffer right, newest sample first (upstream
            // stores reversed; magnitudes are unaffected).
            let len = self.input_buffer.len();
            self.input_buffer.copy_within(0..len - new, new);
            for (n, s) in new_samples.iter().take(new).enumerate() {
                // f32 -1..1 -> int16-scale so the EQ/sens constants match upstream.
                self.input_buffer[new - n - 1] = (*s as f64) * 32767.0;
                if *s != 0.0 {
                    silence = false;
                }
            }
        } else {
            self.frame_skip += 1;
        }

        // Windowed copies (bass uses the longer buffer).
        for i in 0..self.fft_bass_size {
            self.bass_raw[i] = self.bass_multiplier[i] * self.input_buffer[i];
        }
        for i in 0..self.fft_size {
            self.mid_raw[i] = self.multiplier[i] * self.input_buffer[i];
        }

        for i in 0..self.fft_bass_size {
            self.bass_spectrum[i] = Complex64::new(self.bass_raw[i], 0.0);
        }
        for i in 0..self.fft_size {
            self.mid_spectrum[i] = Complex64::new(self.mid_raw[i], 0.0);
        }
        self.fft_bass.process(&mut self.bass_spectrum);
        self.fft_mid.process(&mut self.mid_spectrum);

        // Per-bar averaged magnitudes.
        let mut cava_out = vec![0f64; self.number_of_bars];
        for (n, out_n) in cava_out.iter_mut().enumerate() {
            let mut temp = 0.0;
            for i in self.lower_cut_off[n]..=self.upper_cut_off[n] {
                if n < self.bass_cut_off_bar {
                    temp += self.bass_spectrum[i].norm();
                } else {
                    temp += self.mid_spectrum[i].norm();
                }
            }
            *out_n = temp * self.eq[n];
        }

        if self.autosens {
            for v in cava_out.iter_mut() {
                *v *= self.sens;
            }
        }

        // Smoothing: gravity falloff + integral memory (upstream maths).
        let framerate_mod = 66.0 / self.framerate;
        let gravity_mod = framerate_mod.powf(2.5) * 2.0 / self.noise_reduction;
        let integral_mod = framerate_mod.powf(0.1);
        let mut overshoot = false;

        for (n, out_n) in cava_out.iter_mut().enumerate() {
            if *out_n < self.prev_cava_out[n] && self.noise_reduction > 0.1 {
                *out_n =
                    self.cava_peak[n] * (1.0 - self.cava_fall[n] * self.cava_fall[n] * gravity_mod);
                if *out_n < 0.0 {
                    *out_n = 0.0;
                }
                self.cava_fall[n] += 0.028;
            } else {
                self.cava_peak[n] = *out_n;
                self.cava_fall[n] = 0.0;
            }
            self.prev_cava_out[n] = *out_n;

            *out_n += self.cava_mem[n] * self.noise_reduction / integral_mod;
            self.cava_mem[n] = *out_n;
            if self.autosens && *out_n > 1.0 {
                overshoot = true;
                *out_n = 1.0;
            }
        }

        if self.autosens {
            if overshoot {
                self.sens *= 1.0 - 0.02 * framerate_mod;
                self.sens_init = false;
            } else if !silence {
                self.sens *= 1.0 + 0.001 * framerate_mod * (self.autosens as u8) as f64;
                if self.sens_init {
                    self.sens *= 1.0 + 0.1 * framerate_mod;
                }
            }
        }

        for (o, v) in self.out.iter_mut().zip(cava_out.iter()) {
            *o = *v as f32;
        }
        &self.out
    }

    /// Clear ring buffers, peaks, memory and sensitivity.
    pub fn reset(&mut self) {
        self.input_buffer.fill(0.0);
        self.cava_fall.fill(0.0);
        self.cava_mem.fill(0.0);
        self.cava_peak.fill(0.0);
        self.prev_cava_out.fill(0.0);
        self.out.fill(0.0);
        self.sens = 1.0;
        self.sens_init = true;
        self.framerate = 75.0;
        self.frame_skip = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, rate: u32, n: usize, start: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * PI * freq * (start + i) as f64 / rate as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn silence_gives_zero_bars() {
        let mut c = CavaCore::new(48, 44100, true, 0.77, 40.0, 16000.0);
        let silence = vec![0f32; 2205];
        let mut last = vec![];
        for _ in 0..40 {
            last = c.execute(&silence).to_vec();
        }
        assert!(last.iter().all(|v| *v == 0.0), "got {last:?}");
    }

    #[test]
    fn sine_440_peaks_near_440() {
        let rate = 44100;
        let mut c = CavaCore::new(48, rate, true, 0.77, 40.0, 16000.0);
        let mut bars = vec![];
        let mut pos = 0usize;
        while pos < rate as usize * 2 {
            let chunk = sine(440.0, rate, 2205, pos);
            bars = c.execute(&chunk).to_vec();
            pos += 2205;
        }
        assert!(bars.iter().all(|v| (0.0..=1.0).contains(v)), "got {bars:?}");
        let mut order: Vec<usize> = (0..bars.len()).collect();
        order.sort_by(|a, b| bars[*b].partial_cmp(&bars[*a]).unwrap());
        let freqs = c.bar_freqs().to_vec();
        let closest = (0..freqs.len())
            .min_by(|a, b| {
                (freqs[*a] - 440.0)
                    .abs()
                    .partial_cmp(&(freqs[*b] - 440.0).abs())
                    .unwrap()
            })
            .unwrap();
        assert!(
            order[..2].contains(&closest),
            "closest bar {closest} ({} Hz) not in top2 {order:?} — bars {bars:?}",
            freqs[closest]
        );
    }

    #[test]
    fn reset_clears_bars() {
        let rate = 44100;
        let mut c = CavaCore::new(48, rate, true, 0.77, 40.0, 16000.0);
        let mut pos = 0usize;
        while pos < rate as usize {
            let chunk = sine(440.0, rate, 2205, pos);
            c.execute(&chunk);
            pos += 2205;
        }
        c.reset();
        let silence = vec![0f32; 2205];
        let mut bars = vec![];
        for _ in 0..10 {
            bars = c.execute(&silence).to_vec();
        }
        assert!(bars.iter().all(|v| *v == 0.0), "got {bars:?}");
    }
}
