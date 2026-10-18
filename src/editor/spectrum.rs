use slint::{Rgb8Pixel, SharedPixelBuffer};

use crate::params::{freq_to_pos, pos_to_freq, threshold_at, CURVE_POINTS};
use crate::shared::{SpectrumFrame, DISPLAY_POINTS};

// must match View in the slint file
pub const VIEW_WIDTH: f32 = 508.0;
pub const VIEW_HEIGHT: f32 = 180.0;

const DB_MIN: f32 = -100.0;
const DB_MAX: f32 = 0.0;
const FLOOR: f32 = DB_MIN - 20.0;

type Rgb = [f32; 3];
const BG: Rgb = [30.0, 28.0, 26.0];
const AMBER: Rgb = [255.0, 174.0, 66.0];
const LINE: Rgb = [214.0, 208.0, 196.0];
const GREY: Rgb = [138.0, 133.0, 124.0];

pub fn db_to_y(db: f32) -> f32 {
    ((db - DB_MAX) / (DB_MIN - DB_MAX)).clamp(0.0, 1.0)
}

pub fn y_to_db(y: f32) -> f32 {
    DB_MAX + y * (DB_MIN - DB_MAX)
}

// bigger fft gets smaller cells
fn pitch_for(fft_size: usize) -> f32 {
    match fft_size {
        0..=512 => 15.0,
        513..=1024 => 12.0,
        1025..=2048 => 9.5,
        2049..=4096 => 7.5,
        _ => 6.0,
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct ViewParams {
    pub threshold_db: f32,
    pub tilt: f32,
    pub curve: [f32; CURVE_POINTS],
    pub low_hz: f32,
    pub high_hz: f32,
    pub fft_size: usize,
}

fn shade(c: Rgb, a: f32) -> Rgb8Pixel {
    let m = |i: usize| (BG[i] + (c[i] - BG[i]) * a).round() as u8;
    Rgb8Pixel::new(m(0), m(1), m(2))
}

fn blend(px: &mut Rgb8Pixel, c: Rgb, a: f32) {
    if a <= 0.0 {
        return;
    }
    let a = a.min(1.0);
    px.r = (px.r as f32 + (c[0] - px.r as f32) * a) as u8;
    px.g = (px.g as f32 + (c[1] - px.g as f32) * a) as u8;
    px.b = (px.b as f32 + (c[2] - px.b as f32) * a) as u8;
}

fn peak(data: &[f32; DISPLAY_POINTS], from: f32, to: f32) -> f32 {
    let n = DISPLAY_POINTS as f32;
    let first = ((from.max(0.0) * n) as usize).min(DISPLAY_POINTS - 1);
    let last = ((to * n).ceil() as usize).clamp(first + 1, DISPLAY_POINTS);
    data[first..last]
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max)
}

pub struct SpectrumRenderer {
    input: [f32; DISPLAY_POINTS],
    output: [f32; DISPLAY_POINTS],
    pitch: Option<f32>,
    curve_y: Vec<f32>,
}

impl SpectrumRenderer {
    pub fn new() -> Self {
        Self {
            input: [FLOOR; DISPLAY_POINTS],
            output: [FLOOR; DISPLAY_POINTS],
            pitch: None,
            curve_y: Vec::new(),
        }
    }

    pub fn feed(&mut self, frame: &SpectrumFrame) {
        for p in 0..DISPLAY_POINTS {
            let input = frame.mag_db[p].max(FLOOR);
            let output = (frame.mag_db[p] + 20.0 * frame.gain[p].max(1e-6).log10()).max(FLOOR);
            for (s, v) in [(&mut self.input[p], input), (&mut self.output[p], output)] {
                let k = if v > *s { 0.6 } else { 0.15 };
                *s += (v - *s) * k;
            }
        }
    }

    pub fn animating(&self, view: &ViewParams) -> bool {
        self.pitch
            .is_some_and(|p| (p - pitch_for(view.fft_size)).abs() > 0.01)
    }

    pub fn render(
        &mut self,
        width: usize,
        height: usize,
        view: &ViewParams,
    ) -> SharedPixelBuffer<Rgb8Pixel> {
        let (w, h) = (width.max(1), height.max(1));
        let scale = w as f32 / VIEW_WIDTH;

        let target = pitch_for(view.fft_size);
        let pitch = match self.pitch {
            Some(p) if (p - target).abs() > 0.01 => p + (target - p) * 0.2,
            _ => target,
        };
        self.pitch = Some(pitch);

        let mut img = SharedPixelBuffer::<Rgb8Pixel>::new(w as u32, h as u32);
        let px = img.make_mut_slice();
        px.fill(shade(BG, 0.0));

        let cell = pitch * 0.72;
        let gap = pitch - cell;
        let cols = ((VIEW_WIDTH + gap) / pitch).floor().max(1.0) as usize;
        let rows = ((VIEW_HEIGHT + gap) / pitch).floor().max(1.0) as usize;
        let ox = (VIEW_WIDTH - (cols as f32 * pitch - gap)) / 2.0;
        let oy = (VIEW_HEIGHT - (rows as f32 * pitch - gap)) / 2.0;

        let lo = freq_to_pos(view.low_hz.min(view.high_hz));
        let hi = freq_to_pos(view.high_hz.max(view.low_hz));

        // passed gated empty
        let inside = [shade(AMBER, 1.0), shade(AMBER, 0.2), shade(AMBER, 0.055)];
        let outside = [shade(GREY, 0.55), shade(GREY, 0.16), shade(GREY, 0.04)];

        for c in 0..cols {
            let x = ox + c as f32 * pitch;
            let left = x / VIEW_WIDTH;
            let right = (x + cell) / VIEW_WIDTH;
            let mid = (left + right) * 0.5;
            let in_db = peak(&self.input, left, right);
            let out_db = peak(&self.output, left, right);
            let colors = if mid >= lo && mid <= hi {
                &inside
            } else {
                &outside
            };

            let x0 = (x * scale).round() as usize;
            let x1 = ((x + cell) * scale).round() as usize;
            for r in 0..rows {
                let y = oy + r as f32 * pitch;
                let db = y_to_db((y + cell * 0.5) / VIEW_HEIGHT);
                let color = if out_db >= db {
                    colors[0]
                } else if in_db >= db {
                    colors[1]
                } else {
                    colors[2]
                };
                let y0 = (y * scale).round() as usize;
                let y1 = ((y + cell) * scale).round() as usize;
                fill(px, w, h, (x0, y0), (x1, y1), color);
            }
        }

        self.curve_y.clear();
        for x in 0..w {
            let pos = (x as f32 + 0.5) / w as f32;
            let thr = threshold_at(view.threshold_db, view.tilt, &view.curve, pos_to_freq(pos));
            self.curve_y.push(db_to_y(thr) * h as f32);
        }
        // dark outline first so the line cuts through the cells
        line(px, w, h, &self.curve_y, BG, 0.9, 2.2 * scale);
        line(px, w, h, &self.curve_y, LINE, 1.0, 0.8 * scale);

        img
    }
}

fn fill(
    px: &mut [Rgb8Pixel],
    w: usize,
    h: usize,
    from: (usize, usize),
    to: (usize, usize),
    color: Rgb8Pixel,
) {
    let (x1, y1) = (to.0.min(w), to.1.min(h));
    if from.0 >= x1 {
        return;
    }
    for y in from.1..y1 {
        px[y * w + from.0..y * w + x1].fill(color);
    }
}

// antialiased line with one y per pixel column
fn line(px: &mut [Rgb8Pixel], w: usize, h: usize, ys: &[f32], color: Rgb, alpha: f32, half: f32) {
    let reach = half + 1.0;
    for x in 0..w {
        let (a, b) = (ys[x.saturating_sub(1)], ys[x]);
        let (top, bottom) = if a < b { (a, b) } else { (b, a) };
        let start = (top - reach).floor().max(0.0) as usize;
        let end = ((bottom + reach).ceil() as usize).min(h);
        for y in start..end {
            let yc = y as f32 + 0.5;
            let d = (top - yc).max(yc - bottom).max(0.0);
            let cover = (half + 0.5 - d).clamp(0.0, 1.0);
            blend(&mut px[y * w + x], color, cover * alpha);
        }
    }
}
