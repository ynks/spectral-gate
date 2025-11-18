# Spectral Gate VST Plugin

## Overview

This spectral gate plugin processes audio in the frequency domain using Fast Fourier Transform (FFT) to selectively attenuate or pass spectral components based on their amplitude.

## Features

### Parameters

1. **Cutoff Amplitude** (-60 to 0 dB)
   - Sets the threshold below which spectral components are attenuated
   - Default: -30 dB
   - Lower values result in more aggressive gating

2. **Weak/Strong Balance** (0-100%)
   - Controls the attenuation strength for components below the threshold
   - 0% = Strong gate (full attenuation below threshold)
   - 100% = Weak gate (no attenuation, essentially bypassed)
   - Default: 50%
   - Allows for subtle to aggressive spectral shaping

3. **Dry/Wet** (0-100%)
   - Mixes the processed signal with the original input
   - 0% = Completely dry (original signal)
   - 100% = Completely wet (fully processed signal)
   - Default: 100%
   - Useful for parallel processing techniques

## Technical Implementation

### FFT Processing

- **FFT Size**: 1024 samples (2^10)
- **Hop Size**: 256 samples (75% overlap)
- **Window Function**: Hann window for smooth transitions
- **Processing**: Short-Time Fourier Transform (STFT) with overlap-add synthesis

### Algorithm

1. Input audio is buffered into frames of 1024 samples
2. Each frame is windowed using a Hann window
3. Forward FFT transforms the time-domain signal to frequency domain
4. For each frequency bin:
   - Calculate magnitude from real and imaginary components
   - If magnitude < cutoff threshold:
     - Apply attenuation based on weak/strong balance parameter
   - If magnitude >= cutoff threshold:
     - Pass through unchanged
5. Inverse FFT transforms back to time domain
6. Apply window again for overlap-add
7. Mix with dry signal based on dry/wet parameter

### Latency

The plugin introduces latency due to the FFT processing:
- Processing latency: approximately 1024 samples at the current FFT size
- At 44.1 kHz: ~23ms latency
- At 48 kHz: ~21ms latency

## Usage Examples

### Noise Reduction
- Cutoff: -40 dB
- Balance: 20%
- Dry/Wet: 70%

This setting creates a gentle noise gate that reduces low-level noise while preserving most of the original signal character.

### Creative Sound Design
- Cutoff: -20 dB
- Balance: 0%
- Dry/Wet: 100%

This creates an aggressive spectral gate that can produce interesting rhythmic and textural effects.

### Subtle Enhancement
- Cutoff: -50 dB
- Balance: 80%
- Dry/Wet: 30%

This provides very gentle gating mixed subtly with the original signal, useful for cleaning up quiet sections without obvious processing.

## Building

The plugin is built using CMake and JUCE. See the main README for build instructions.

## Testing

Tests are located in `tests/PluginBasics.cpp` and include:
- Parameter validation tests
- Audio processing tests
- Pipeline initialization tests

Run tests with:
```bash
cd build
./Tests
```

## Technical Notes

- The plugin processes each channel independently
- State is saved/loaded using JUCE's AudioProcessorValueTreeState
- UI updates are thread-safe via parameter attachments
- FFT processing uses JUCE's dsp::FFT class
- Overlap-add synthesis ensures smooth audio output without artifacts
