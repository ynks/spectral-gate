use realfft::num_complex::Complex32;

use super::stft::MAX_BINS;
use crate::params::{threshold_at, GateMode, CURVE_POINTS};

#[derive(Clone, Copy)]
pub struct GateSettings {
    pub mode: GateMode,
    pub threshold_db: f32,
    pub tilt: f32,
    pub curve: [f32; CURVE_POINTS],
    pub hysteresis_db: f32,
    pub floor_gain: f32,
    pub attack_coef: f32,
    pub release_coef: f32,
    pub low_hz: f32,
    pub high_hz: f32,
    pub link: f32,
    pub freeze: bool,
    pub density: f32,
}

pub fn hop_coef(ms: f32, hop: usize, sr: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-(hop as f32) / (sr * ms * 0.001)).exp()
    }
}

type ThresholdKey = (usize, f32, f32, f32, [f32; CURVE_POINTS]);

pub struct Gate {
    gains: Vec<Vec<f32>>,
    open: Vec<Vec<bool>>,
    mag_db: Vec<Vec<f32>>,
    thresholds: Vec<f32>,
    threshold_key: Option<ThresholdKey>,
    seed: u32,
}

impl Gate {
    pub fn new(channels: usize) -> Self {
        Self {
            gains: vec![vec![1.0; MAX_BINS]; channels],
            open: vec![vec![true; MAX_BINS]; channels],
            mag_db: vec![vec![-200.0; MAX_BINS]; channels],
            thresholds: vec![0.0; MAX_BINS],
            threshold_key: None,
            seed: 0x9e37_79b9,
        }
    }

    pub fn reset(&mut self) {
        self.gains.iter_mut().for_each(|g| g.fill(1.0));
        self.open.iter_mut().for_each(|o| o.fill(true));
        self.mag_db.iter_mut().for_each(|m| m.fill(-200.0));
    }

    pub fn mag_db(&self, ch: usize) -> &[f32] {
        &self.mag_db[ch]
    }

    pub fn gains(&self, ch: usize) -> &[f32] {
        &self.gains[ch]
    }

    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn update_thresholds(&mut self, s: &GateSettings, bins: usize, sr: f32) {
        let key = (bins, sr, s.threshold_db, s.tilt, s.curve);
        if self.threshold_key == Some(key) {
            return;
        }
        self.threshold_key = Some(key);

        let bin_hz = sr / ((bins - 1) * 2) as f32;
        for (k, t) in self.thresholds[..bins].iter_mut().enumerate() {
            let freq = (k as f32 * bin_hz).max(bin_hz * 0.5);
            *t = threshold_at(s.threshold_db, s.tilt, &s.curve, freq);
        }
    }

    pub fn process(
        &mut self,
        spectra: &mut [Vec<Complex32>],
        bins: usize,
        mag_scale: f32,
        sr: f32,
        s: &GateSettings,
    ) {
        let channels = spectra.len().min(self.gains.len());
        self.update_thresholds(s, bins, sr);

        for ch in 0..channels {
            for (m, c) in self.mag_db[ch][..bins].iter_mut().zip(&spectra[ch][..bins]) {
                *m = 20.0 * (c.norm() * mag_scale + 1e-10).log10();
            }
        }

        let bin_hz = sr / ((bins - 1) * 2) as f32;
        let low = (s.low_hz / bin_hz).floor().max(0.0) as usize;
        let high = ((s.high_hz / bin_hz).ceil() as usize).min(bins - 1);

        for k in 0..bins {
            // low and high cut remove everything outside
            if k < low || k > high {
                for ch in 0..channels {
                    spectra[ch][k] = Complex32::default();
                    self.gains[ch][k] = 0.0;
                }
                continue;
            }

            let loudest = (0..channels)
                .map(|ch| self.mag_db[ch][k])
                .fold(f32::NEG_INFINITY, f32::max);
            let thresh = self.thresholds[k];
            let dropped = s.density < 1.0 && self.random() >= s.density;

            for ch in 0..channels {
                if !s.freeze {
                    let own = self.mag_db[ch][k];
                    let level = own + (loudest - own) * s.link;
                    let was_open = self.open[ch][k];

                    let open = match s.mode {
                        GateMode::Below if was_open => level > thresh - s.hysteresis_db,
                        GateMode::Below => level > thresh,
                        GateMode::Above if was_open => level < thresh + s.hysteresis_db,
                        GateMode::Above => level < thresh,
                    };
                    self.open[ch][k] = open;

                    let target = if open && !dropped { 1.0 } else { s.floor_gain };
                    let g = &mut self.gains[ch][k];
                    let coef = if target > *g {
                        s.attack_coef
                    } else {
                        s.release_coef
                    };
                    *g = target + (*g - target) * coef;
                }

                spectra[ch][k] *= self.gains[ch][k];
            }
        }
    }
}
