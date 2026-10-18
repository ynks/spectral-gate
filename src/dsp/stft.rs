use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::f32::consts::TAU;
use std::sync::Arc;

pub const MIN_ORDER: usize = 9;
pub const MAX_ORDER: usize = 13;
pub const MAX_SIZE: usize = 1 << MAX_ORDER;
pub const MAX_BINS: usize = MAX_SIZE / 2 + 1;

// sized for 8192 so resizing never allocates
pub struct Stft {
    channels: usize,
    size: usize,
    hop: usize,

    fwd: Vec<Arc<dyn RealToComplex<f32>>>,
    inv: Vec<Arc<dyn ComplexToReal<f32>>>,

    window: Vec<f32>,
    norm: f32,
    mag_scale: f32,

    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    pos: usize,
    count: usize,

    time: Vec<f32>,
    spectra: Vec<Vec<Complex32>>,
    scratch: Vec<Complex32>,
}

impl Stft {
    pub fn new(channels: usize) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let mut fwd = Vec::new();
        let mut inv = Vec::new();
        for order in MIN_ORDER..=MAX_ORDER {
            fwd.push(planner.plan_fft_forward(1 << order));
            inv.push(planner.plan_fft_inverse(1 << order));
        }
        let scratch_len = fwd
            .iter()
            .map(|f| f.get_scratch_len())
            .chain(inv.iter().map(|f| f.get_scratch_len()))
            .max()
            .unwrap_or(0);

        let mut stft = Self {
            channels,
            size: 0,
            hop: 0,
            fwd,
            inv,
            window: vec![0.0; MAX_SIZE],
            norm: 1.0,
            mag_scale: 1.0,
            input: vec![vec![0.0; MAX_SIZE]; channels],
            output: vec![vec![0.0; MAX_SIZE]; channels],
            pos: 0,
            count: 0,
            time: vec![0.0; MAX_SIZE],
            spectra: vec![vec![Complex32::default(); MAX_BINS]; channels],
            scratch: vec![Complex32::default(); scratch_len],
        };
        stft.configure(2048, 4);
        stft
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn hop(&self) -> usize {
        self.hop
    }

    pub fn num_bins(&self) -> usize {
        self.size / 2 + 1
    }

    pub fn latency(&self) -> usize {
        self.size
    }

    pub fn mag_scale(&self) -> f32 {
        self.mag_scale
    }

    pub fn configure(&mut self, size: usize, overlap: usize) {
        let size = size.clamp(1 << MIN_ORDER, MAX_SIZE);
        let hop = size / overlap.max(2);
        if size == self.size && hop == self.hop {
            return;
        }
        self.size = size;
        self.hop = hop;

        // sqrt hann at 2x so it still sums flat
        let sqrt = overlap <= 2;
        let mut sum = 0.0;
        for (i, w) in self.window[..size].iter_mut().enumerate() {
            let hann = 0.5 - 0.5 * (TAU * i as f32 / size as f32).cos();
            *w = if sqrt { hann.sqrt() } else { hann };
            sum += *w;
        }
        // full scale sine reads 0 dB
        self.mag_scale = 2.0 / sum;

        let mut ola = 0.0;
        for i in 0..hop {
            let mut j = i;
            while j < size {
                ola += self.window[j] * self.window[j];
                j += hop;
            }
        }
        ola /= hop as f32;
        self.norm = 1.0 / (ola * size as f32);

        self.reset();
    }

    pub fn reset(&mut self) {
        for buf in self.input.iter_mut().chain(self.output.iter_mut()) {
            buf.fill(0.0);
        }
        self.pos = 0;
        self.count = 0;
    }

    // dry is the input delayed by the latency
    #[inline]
    pub fn tick<F>(&mut self, io: &mut [f32], dry: &mut [f32], mut process: F)
    where
        F: FnMut(&mut [Vec<Complex32>], usize),
    {
        let pos = self.pos;
        for ch in 0..self.channels {
            dry[ch] = self.input[ch][pos];
            self.input[ch][pos] = io[ch];
            io[ch] = self.output[ch][pos];
            self.output[ch][pos] = 0.0;
        }
        self.pos = (pos + 1) & (self.size - 1);

        self.count += 1;
        if self.count >= self.hop {
            self.count = 0;
            self.frame(&mut process);
        }
    }

    fn frame<F>(&mut self, process: &mut F)
    where
        F: FnMut(&mut [Vec<Complex32>], usize),
    {
        let n = self.size;
        let mask = n - 1;
        let bins = n / 2 + 1;
        let plan = n.trailing_zeros() as usize - MIN_ORDER;

        for ch in 0..self.channels {
            for i in 0..n {
                self.time[i] = self.input[ch][(self.pos + i) & mask] * self.window[i];
            }
            let _ = self.fwd[plan].process_with_scratch(
                &mut self.time[..n],
                &mut self.spectra[ch][..bins],
                &mut self.scratch,
            );
        }

        process(&mut self.spectra, bins);

        for ch in 0..self.channels {
            let spectrum = &mut self.spectra[ch][..bins];
            // realfft wants dc and nyquist purely real
            spectrum[0].im = 0.0;
            spectrum[bins - 1].im = 0.0;
            let _ = self.inv[plan].process_with_scratch(
                spectrum,
                &mut self.time[..n],
                &mut self.scratch,
            );

            let out = &mut self.output[ch];
            for i in 0..n {
                out[(self.pos + i) & mask] += self.time[i] * self.window[i] * self.norm;
            }
        }
    }
}
