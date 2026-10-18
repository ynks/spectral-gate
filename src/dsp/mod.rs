pub mod gate;
pub mod stft;

#[cfg(test)]
mod tests {
    use super::gate::{Gate, GateSettings};
    use super::stft::Stft;
    use crate::params::{GateMode, CURVE_POINTS};
    use std::f32::consts::TAU;

    const SR: f32 = 48000.0;

    fn settings(threshold_db: f32) -> GateSettings {
        GateSettings {
            mode: GateMode::Below,
            threshold_db,
            tilt: 0.0,
            curve: [0.0; CURVE_POINTS],
            hysteresis_db: 0.0,
            floor_gain: 0.0,
            attack_coef: 0.0,
            release_coef: 0.0,
            low_hz: 0.0,
            high_hz: SR,
            link: 1.0,
            freeze: false,
            density: 1.0,
        }
    }

    fn sine(i: usize, freq: f32, amp: f32) -> f32 {
        amp * (TAU * freq * i as f32 / SR).sin()
    }

    fn run(
        size: usize,
        overlap: usize,
        s: GateSettings,
        len: usize,
        f: impl Fn(usize) -> f32,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut stft = Stft::new(1);
        stft.configure(size, overlap);
        let mut gate = Gate::new(1);
        let scale = stft.mag_scale();

        let input: Vec<f32> = (0..len).map(f).collect();
        let mut output = Vec::with_capacity(len);
        let mut dry = [0.0];
        for &x in &input {
            let mut io = [x];
            stft.tick(&mut io, &mut dry, |spec, bins| {
                gate.process(spec, bins, scale, SR, &s)
            });
            output.push(io[0]);
        }
        (input, output)
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0f32, |a, &b| a.max(b.abs()))
    }

    #[test]
    fn open_gate_is_transparent() {
        for (size, overlap) in [(512, 2), (2048, 4), (8192, 8)] {
            let (input, output) = run(size, overlap, settings(-200.0), size * 6, |i| {
                sine(i, 440.0, 0.5) + sine(i, 3000.0, 0.25)
            });
            let err = (size * 2..input.len())
                .map(|i| (output[i] - input[i - size]).abs())
                .fold(0.0, f32::max);
            assert!(err < 1e-4, "{size} {overlap}x err {err}");
        }
    }

    #[test]
    fn quiet_sine_gets_gated() {
        let (_, output) = run(2048, 4, settings(-20.0), 2048 * 8, |i| {
            sine(i, 1000.0, 0.01)
        });
        assert!(peak(&output[2048 * 4..]) < 1e-5);
    }

    #[test]
    fn loud_sine_passes() {
        let (input, output) = run(2048, 4, settings(-40.0), 2048 * 8, |i| sine(i, 1000.0, 0.5));
        let err = (2048 * 4..input.len())
            .map(|i| (output[i] - input[i - 2048]).abs())
            .fold(0.0, f32::max);
        // the leakage skirts still get gated
        assert!(err < 5e-3, "err {err}");
    }

    #[test]
    fn above_mode_removes_loud_sine() {
        let mut s = settings(-40.0);
        s.mode = GateMode::Above;
        let (_, output) = run(2048, 4, s, 2048 * 8, |i| sine(i, 1000.0, 0.5));
        assert!(peak(&output[2048 * 4..]) < 0.01);
    }

    #[test]
    fn low_cut_removes_lows() {
        let mut s = settings(-200.0);
        s.low_hz = 1000.0;
        let (_, output) = run(2048, 4, s, 2048 * 8, |i| sine(i, 100.0, 0.5));
        assert!(peak(&output[2048 * 4..]) < 1e-3);
    }

    #[test]
    fn density_thins_things_out() {
        let energy = |density: f32| {
            let mut s = settings(-200.0);
            s.density = density;
            let (_, output) = run(2048, 4, s, 2048 * 16, |i| {
                sine(i, 1000.0, 0.5) + sine(i, 3000.0, 0.3)
            });
            output[2048 * 4..].iter().map(|x| x * x).sum::<f32>()
        };
        let full = energy(1.0);
        let half = energy(0.5);
        assert!(energy(0.0) < full * 1e-6);
        assert!(half > full * 0.05 && half < full * 0.8, "{half} vs {full}");
    }

    #[test]
    fn full_scale_sine_is_0db() {
        let mut stft = Stft::new(1);
        stft.configure(4096, 4);
        let mut gate = Gate::new(1);
        let scale = stft.mag_scale();
        let s = settings(-200.0);
        let mut dry = [0.0];
        let freq = 100.0 * SR / 4096.0;
        for i in 0..4096 * 3 {
            let mut io = [sine(i, freq, 1.0)];
            stft.tick(&mut io, &mut dry, |spec, bins| {
                gate.process(spec, bins, scale, SR, &s)
            });
        }
        let db = gate.mag_db(0)[100];
        assert!(db.abs() < 0.1, "{db} dB");
    }
}
