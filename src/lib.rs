use nih_plug::prelude::*;
use std::sync::{Arc, Mutex};

mod dsp;
mod editor;
mod params;
mod shared;

use dsp::gate::{hop_coef, Gate, GateSettings};
use dsp::stft::Stft;
use params::SgParams;
use shared::{DisplayMap, SpectrumFrame};

pub struct SpectralGate {
    params: Arc<SgParams>,
    sample_rate: f32,
    channels: usize,

    stft: Stft,
    gate: Gate,

    display_map: DisplayMap,
    frame_in: triple_buffer::Input<SpectrumFrame>,
    frame_out: Arc<Mutex<triple_buffer::Output<SpectrumFrame>>>,

    // test signal for the standalone build
    #[cfg(feature = "standalone")]
    demo: Option<demo::Demo>,
}

#[cfg(feature = "standalone")]
mod demo {
    use std::f32::consts::TAU;

    pub struct Demo {
        phase: [f32; 4],
        lfo: f32,
        seed: u32,
    }

    impl Demo {
        pub fn new() -> Self {
            Self {
                phase: [0.0; 4],
                lfo: 0.0,
                seed: 0x1234_5678,
            }
        }

        pub fn next(&mut self, sr: f32) -> f32 {
            const FREQS: [f32; 4] = [110.0, 440.0, 1250.0, 3600.0];
            const AMPS: [f32; 4] = [0.25, 0.12, 0.05, 0.02];

            self.lfo = (self.lfo + 0.25 / sr).fract();
            let mut out = 0.0;
            for (i, p) in self.phase.iter_mut().enumerate() {
                *p = (*p + FREQS[i] / sr).fract();
                let trem = 0.5 + 0.5 * (TAU * (self.lfo + i as f32 * 0.25)).sin();
                out += (TAU * *p).sin() * AMPS[i] * trem;
            }

            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            out + (self.seed as f32 / u32::MAX as f32 - 0.5) * 0.004
        }
    }
}

impl Default for SpectralGate {
    fn default() -> Self {
        let (frame_in, frame_out) = triple_buffer::triple_buffer(&SpectrumFrame::default());
        Self {
            params: Arc::new(SgParams::default()),
            sample_rate: 48000.0,
            channels: 2,
            stft: Stft::new(2),
            gate: Gate::new(2),
            display_map: DisplayMap::new(),
            frame_in,
            frame_out: Arc::new(Mutex::new(frame_out)),
            #[cfg(feature = "standalone")]
            demo: std::env::var_os("SG_DEMO").map(|_| demo::Demo::new()),
        }
    }
}

impl SpectralGate {
    fn update_stft(&mut self) -> bool {
        let size = self.params.fft_size.value().samples();
        let overlap = self.params.overlap.value().factor();
        let old = (self.stft.size(), self.stft.hop());
        self.stft.configure(size, overlap);

        if old == (self.stft.size(), self.stft.hop()) {
            return false;
        }
        self.gate.reset();
        self.display_map
            .update(self.stft.num_bins(), self.sample_rate);
        true
    }

    fn gate_settings(&self) -> GateSettings {
        let p = &self.params;
        let hop = self.stft.hop();
        let depth = p.depth.value();
        let (low, high) = (p.low_freq.value(), p.high_freq.value());

        GateSettings {
            mode: p.mode.value(),
            threshold_db: p.threshold.value(),
            tilt: p.tilt.value(),
            curve: p.curve_offsets(),
            hysteresis_db: p.hysteresis.value(),
            floor_gain: if depth <= -79.95 {
                0.0
            } else {
                util::db_to_gain(depth)
            },
            attack_coef: hop_coef(p.attack.value(), hop, self.sample_rate),
            release_coef: hop_coef(p.release.value(), hop, self.sample_rate),
            low_hz: low.min(high),
            high_hz: high.max(low),
            link: p.stereo_link.value(),
            freeze: p.freeze.value(),
            density: p.density.value(),
        }
    }
}

impl Plugin for SpectralGate {
    const NAME: &'static str = "Spectral Gate";
    const VENDOR: &'static str = "xein";
    const URL: &'static str = "www.xein.es";
    const EMAIL: &'static str = "xgonip@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.frame_out.clone())
    }

    fn initialize(
        &mut self,
        layout: &AudioIOLayout,
        config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = config.sample_rate;
        self.channels = layout
            .main_output_channels
            .map(NonZeroU32::get)
            .unwrap_or(2) as usize;
        self.stft = Stft::new(self.channels);
        self.gate = Gate::new(self.channels);
        self.update_stft();
        self.display_map
            .update(self.stft.num_bins(), self.sample_rate);
        context.set_latency_samples(self.stft.latency() as u32);
        true
    }

    fn reset(&mut self) {
        self.stft.reset();
        self.gate.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if self.update_stft() {
            context.set_latency_samples(self.stft.latency() as u32);
        }

        let settings = self.gate_settings();
        let mag_scale = self.stft.mag_scale();
        let sr = self.sample_rate;
        let channels = self.channels.min(buffer.channels());
        let mut new_frame = false;

        for mut samples in buffer.iter_samples() {
            let mix = self.params.mix.smoothed.next();
            let gain = self.params.output_gain.smoothed.next();

            let mut io = [0.0f32; 2];
            let mut dry = [0.0f32; 2];
            #[cfg(feature = "standalone")]
            let demo = self.demo.as_mut().map(|d| d.next(sr));

            for (c, s) in samples.iter_mut().take(channels).enumerate() {
                #[cfg(feature = "standalone")]
                if let Some(d) = demo {
                    *s = d;
                }
                io[c] = *s;
            }

            let gate = &mut self.gate;
            self.stft.tick(
                &mut io[..channels],
                &mut dry[..channels],
                |spectra, bins| {
                    gate.process(spectra, bins, mag_scale, sr, &settings);
                    new_frame = true;
                },
            );

            for (c, s) in samples.iter_mut().take(channels).enumerate() {
                *s = (dry[c] + (io[c] - dry[c]) * mix) * gain;
            }
        }

        if new_frame && self.params.editor_state.is_open() {
            let bins = self.stft.num_bins();
            let last = channels - 1;
            let mags = [
                &self.gate.mag_db(0)[..bins],
                &self.gate.mag_db(last)[..bins],
            ];
            let gains = [&self.gate.gains(0)[..bins], &self.gate.gains(last)[..bins]];

            self.display_map.fill(
                self.frame_in.input_buffer_mut(),
                &mags[..channels],
                &gains[..channels],
            );
            self.frame_in.publish();
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for SpectralGate {
    const VST3_CLASS_ID: [u8; 16] = *b"XgoniSpectralGt1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Restoration,
        Vst3SubCategory::Analyzer,
    ];
}

nih_export_vst3!(SpectralGate);
