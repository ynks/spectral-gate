use nih_plug::prelude::*;
use std::sync::Arc;

use crate::editor::EditorState;

pub const CURVE_POINTS: usize = 16;
// curve handle range in dB
pub const CURVE_RANGE_DB: f32 = 30.0;
pub const MIN_FREQ: f32 = 20.0;
pub const MAX_FREQ: f32 = 20_000.0;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMode {
    #[name = "Below"]
    Below,
    #[name = "Above"]
    Above,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FftSize {
    #[name = "512"]
    S512,
    #[name = "1024"]
    S1024,
    #[name = "2048"]
    S2048,
    #[name = "4096"]
    S4096,
    #[name = "8192"]
    S8192,
}

impl FftSize {
    pub fn samples(self) -> usize {
        match self {
            FftSize::S512 => 512,
            FftSize::S1024 => 1024,
            FftSize::S2048 => 2048,
            FftSize::S4096 => 4096,
            FftSize::S8192 => 8192,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlap {
    #[name = "2x"]
    X2,
    #[name = "4x"]
    X4,
    #[name = "8x"]
    X8,
}

impl Overlap {
    pub fn factor(self) -> usize {
        match self {
            Overlap::X2 => 2,
            Overlap::X4 => 4,
            Overlap::X8 => 8,
        }
    }
}

#[derive(Params)]
pub struct CurvePoint {
    #[id = "curve"]
    pub offset: FloatParam,
}

impl Default for CurvePoint {
    fn default() -> Self {
        Self {
            offset: FloatParam::new(
                "Curve",
                0.0,
                FloatRange::Linear {
                    min: -CURVE_RANGE_DB,
                    max: CURVE_RANGE_DB,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1)
            .hide_in_generic_ui(),
        }
    }
}

#[derive(Params)]
pub struct SgParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EditorState>,

    #[id = "thresh"]
    pub threshold: FloatParam,
    #[id = "mode"]
    pub mode: EnumParam<GateMode>,
    #[id = "fftsize"]
    pub fft_size: EnumParam<FftSize>,
    #[id = "overlap"]
    pub overlap: EnumParam<Overlap>,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
    #[id = "hyst"]
    pub hysteresis: FloatParam,
    #[id = "depth"]
    pub depth: FloatParam,
    #[id = "tilt"]
    pub tilt: FloatParam,
    #[id = "lowfreq"]
    pub low_freq: FloatParam,
    #[id = "highfreq"]
    pub high_freq: FloatParam,
    #[id = "link"]
    pub stereo_link: FloatParam,
    #[id = "freeze"]
    pub freeze: BoolParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "output"]
    pub output_gain: FloatParam,
    #[id = "density"]
    pub density: FloatParam,

    #[nested(array, group = "Threshold Curve")]
    pub curve: [CurvePoint; CURVE_POINTS],
}

fn ms_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min,
            max,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_unit(" ms")
    .with_value_to_string(Arc::new(|v| {
        if v < 10.0 {
            format!("{v:.1}")
        } else {
            format!("{v:.0}")
        }
    }))
}

fn percent_param(name: &str) -> FloatParam {
    FloatParam::new(name, 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

fn hz_param(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: MIN_FREQ,
            max: MAX_FREQ,
            factor: FloatRange::skew_factor(-2.2),
        },
    )
    .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
    .with_string_to_value(formatters::s2v_f32_hz_then_khz())
}

impl Default for SgParams {
    fn default() -> Self {
        Self {
            editor_state: EditorState::from_size(crate::editor::DEFAULT_SIZE),

            threshold: FloatParam::new(
                "Threshold",
                -50.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),
            mode: EnumParam::new("Mode", GateMode::Below),
            fft_size: EnumParam::new("FFT Size", FftSize::S2048),
            overlap: EnumParam::new("Overlap", Overlap::X4),
            attack: ms_param("Attack", 5.0, 0.0, 500.0),
            release: ms_param("Release", 80.0, 0.0, 2000.0),
            hysteresis: FloatParam::new(
                "Hysteresis",
                3.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),
            depth: FloatParam::new(
                "Depth",
                -80.0,
                FloatRange::Linear {
                    min: -80.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1)
            .with_value_to_string(Arc::new(|v| {
                if v <= -79.95 {
                    String::from("-inf")
                } else {
                    format!("{v:.1}")
                }
            })),
            tilt: FloatParam::new(
                "Tilt",
                0.0,
                FloatRange::Linear {
                    min: -6.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB/oct")
            .with_step_size(0.1),
            low_freq: hz_param("Low Cut", MIN_FREQ),
            high_freq: hz_param("High Cut", MAX_FREQ),
            stereo_link: percent_param("Stereo Link"),
            freeze: BoolParam::new("Freeze", false),
            mix: percent_param("Mix").with_smoother(SmoothingStyle::Linear(30.0)),
            output_gain: FloatParam::new(
                "Output",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-24.0),
                    max: util::db_to_gain(24.0),
                    factor: FloatRange::gain_skew_factor(-24.0, 24.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            density: FloatParam::new(
                "Density",
                1.0,
                // useful range is mostly under 10 percent
                FloatRange::Skewed {
                    min: 0.0,
                    max: 1.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            curve: Default::default(),
        }
    }
}

impl SgParams {
    // same order as P in the slint theme
    pub fn ui_order(&self) -> Vec<ParamPtr> {
        let mut v = vec![
            self.threshold.as_ptr(),
            self.mode.as_ptr(),
            self.fft_size.as_ptr(),
            self.overlap.as_ptr(),
            self.attack.as_ptr(),
            self.release.as_ptr(),
            self.hysteresis.as_ptr(),
            self.depth.as_ptr(),
            self.tilt.as_ptr(),
            self.low_freq.as_ptr(),
            self.high_freq.as_ptr(),
            self.stereo_link.as_ptr(),
            self.freeze.as_ptr(),
            self.mix.as_ptr(),
            self.output_gain.as_ptr(),
            self.density.as_ptr(),
        ];
        v.extend(self.curve.iter().map(|p| p.offset.as_ptr()));
        v
    }

    pub fn curve_offsets(&self) -> [f32; CURVE_POINTS] {
        std::array::from_fn(|i| self.curve[i].offset.value())
    }
}

pub fn curve_point_pos(i: usize) -> f32 {
    i as f32 / (CURVE_POINTS - 1) as f32
}

// log axis from 20 Hz to 20 kHz
pub fn freq_to_pos(freq: f32) -> f32 {
    (freq.max(1.0) / MIN_FREQ).ln() / (MAX_FREQ / MIN_FREQ).ln()
}

pub fn pos_to_freq(pos: f32) -> f32 {
    MIN_FREQ * (MAX_FREQ / MIN_FREQ).powf(pos)
}

// catmull rom through the handles
pub fn curve_at(offsets: &[f32; CURVE_POINTS], pos: f32) -> f32 {
    let x = pos.clamp(0.0, 1.0) * (CURVE_POINTS - 1) as f32;
    let i = (x.floor() as usize).min(CURVE_POINTS - 2);
    let t = x - i as f32;
    let p0 = offsets[i.saturating_sub(1)];
    let p1 = offsets[i];
    let p2 = offsets[i + 1];
    let p3 = offsets[(i + 2).min(CURVE_POINTS - 1)];
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

// tilt pivots around 1 kHz
pub fn threshold_at(base: f32, tilt: f32, offsets: &[f32; CURVE_POINTS], freq: f32) -> f32 {
    base + tilt * (freq.max(1.0) / 1000.0).log2() + curve_at(offsets, freq_to_pos(freq))
}
