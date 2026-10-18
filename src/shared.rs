use crate::params::{pos_to_freq, MAX_FREQ, MIN_FREQ};

pub const DISPLAY_POINTS: usize = 320;

#[derive(Clone)]
pub struct SpectrumFrame {
    pub mag_db: [f32; DISPLAY_POINTS],
    pub gain: [f32; DISPLAY_POINTS],
}

impl Default for SpectrumFrame {
    fn default() -> Self {
        Self {
            mag_db: [-200.0; DISPLAY_POINTS],
            gain: [1.0; DISPLAY_POINTS],
        }
    }
}

// bins to log spaced display points
pub struct DisplayMap {
    // first last and center bin
    ranges: Vec<(usize, usize, f32)>,
    key: (usize, u32),
}

impl DisplayMap {
    pub fn new() -> Self {
        Self {
            ranges: Vec::with_capacity(DISPLAY_POINTS),
            key: (0, 0),
        }
    }

    pub fn update(&mut self, bins: usize, sr: f32) {
        if self.key == (bins, sr.to_bits()) {
            return;
        }
        self.key = (bins, sr.to_bits());
        self.ranges.clear();

        let bin_hz = sr / ((bins - 1) * 2) as f32;
        let half = 0.5 / DISPLAY_POINTS as f32;
        for p in 0..DISPLAY_POINTS {
            let pos = (p as f32 + 0.5) / DISPLAY_POINTS as f32;
            let lo = pos_to_freq((pos - half).max(0.0)).max(MIN_FREQ) / bin_hz;
            let hi = pos_to_freq((pos + half).min(1.0)).min(MAX_FREQ) / bin_hz;
            let center = (pos_to_freq(pos) / bin_hz).min((bins - 1) as f32);
            let first = lo.ceil() as usize;
            let last = (hi.floor() as usize).min(bins - 1);
            self.ranges.push((first, last, center));
        }
    }

    pub fn fill(&self, frame: &mut SpectrumFrame, mags: &[&[f32]], gains: &[&[f32]]) {
        let n = mags.len() as f32;

        for (p, &(first, last, center)) in self.ranges.iter().enumerate() {
            let (m, g) = if first <= last {
                let mut m = f32::NEG_INFINITY;
                let mut g = 0.0;
                for k in first..=last {
                    let mk: f32 = mags.iter().map(|ch| ch[k]).sum();
                    let gk: f32 = gains.iter().map(|ch| ch[k]).sum();
                    m = m.max(mk / n);
                    g += gk / n;
                }
                (m, g / (last - first + 1) as f32)
            } else {
                // low end has fewer bins than points so interpolate
                let k = center.floor() as usize;
                let t = center - k as f32;
                let mut m = 0.0;
                let mut g = 0.0;
                for (mag, gain) in mags.iter().zip(gains) {
                    let k1 = (k + 1).min(mag.len() - 1);
                    m += mag[k] + (mag[k1] - mag[k]) * t;
                    g += gain[k] + (gain[k1] - gain[k]) * t;
                }
                (m / n, g / n)
            };
            frame.mag_db[p] = m;
            frame.gain[p] = g;
        }
    }
}
