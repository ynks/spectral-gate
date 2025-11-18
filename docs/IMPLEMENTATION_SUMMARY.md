# Spectral Gate VST Plugin - Implementation Summary

## Overview
This repository contains a fully functional spectral gate VST plugin built with JUCE framework. The plugin processes audio in the frequency domain to selectively attenuate spectral components based on their amplitude.

## Features Implemented

### 1. Three Control Parameters
- **Cutoff Amplitude** (-60 to 0 dB, default: -30 dB)
  - Sets the threshold below which spectral components are attenuated
  - Displayed in decibels with one decimal place precision
  
- **Weak/Strong Balance** (0-100%, default: 50%)
  - Controls attenuation strength for components below threshold
  - 0% = Strong gate (full attenuation)
  - 100% = Weak gate (no attenuation)
  
- **Dry/Wet Mix** (0-100%, default: 100%)
  - Mixes processed signal with original input
  - 0% = Completely dry
  - 100% = Completely wet

### 2. DSP Implementation
- **FFT Processing**: Uses JUCE's dsp::FFT for efficient frequency domain processing
- **FFT Size**: 1024 samples (2^10)
- **Hop Size**: 256 samples (75% overlap)
- **Windowing**: Hann window for smooth transitions
- **Synthesis**: Overlap-add method for artifact-free output
- **Processing**: Real-time STFT (Short-Time Fourier Transform)

### 3. User Interface
- Modern, clean design with dark background
- Three rotary knobs with value displays
- Labels for each parameter
- Inspector tool for UI debugging
- Window size: 600x400 pixels

### 4. Plugin Infrastructure
- **Parameter Management**: AudioProcessorValueTreeState for thread-safe parameter handling
- **State Management**: Full save/load support for DAW automation
- **Format Support**: VST3, AU, CLAP, and Standalone
- **Latency**: ~23ms at 44.1kHz (1024 samples)

## Project Structure

```
spectral-gate/
├── source/
│   ├── PluginProcessor.h/cpp    # Main DSP logic and parameter handling
│   └── PluginEditor.h/cpp       # UI implementation
├── tests/
│   └── PluginBasics.cpp         # Unit tests
├── docs/
│   ├── SPECTRAL_GATE.md         # Technical documentation
│   └── UI_LAYOUT.md             # UI design documentation
└── build/                       # Build artifacts (gitignored)
```

## Building the Plugin

### Prerequisites
- CMake 3.25 or higher
- C++20 compatible compiler
- JUCE dependencies (installed via submodules)

### Build Steps
```bash
# Clone with submodules
git clone --recursive https://github.com/ynks/spectral-gate.git
cd spectral-gate

# Initialize submodules (if not using --recursive)
git submodule update --init --recursive

# Create build directory
mkdir build && cd build

# Configure
cmake -DCMAKE_BUILD_TYPE=Release ..

# Build
cmake --build . --config Release -j$(nproc)
```

### Build Outputs
- **VST3**: `build/Pamplejuce_artefacts/Release/VST3/`
- **AU**: `build/Pamplejuce_artefacts/Release/AU/`
- **CLAP**: `build/Pamplejuce_artefacts/Release/CLAP/`
- **Standalone**: `build/Pamplejuce_artefacts/Release/Standalone/`

## Testing

Run the test suite:
```bash
cd build
./Tests
```

All tests pass, verifying:
- Parameter instantiation and access
- Audio processing pipeline
- FFT frame processing
- Dry/wet mixing

## Usage Examples

### Noise Reduction
```
Cutoff: -40 dB
Balance: 20%
Dry/Wet: 70%
```
Gentle noise gate that reduces low-level noise while preserving signal character.

### Creative Sound Design
```
Cutoff: -20 dB
Balance: 0%
Dry/Wet: 100%
```
Aggressive spectral gate for rhythmic and textural effects.

### Subtle Enhancement
```
Cutoff: -50 dB
Balance: 80%
Dry/Wet: 30%
```
Very gentle gating mixed subtly with original signal.

## Technical Details

### Algorithm Flow
1. Buffer input audio into 1024-sample frames
2. Apply Hann window
3. Perform forward FFT to frequency domain
4. For each frequency bin:
   - Calculate magnitude from real/imaginary components
   - If magnitude < cutoff: attenuate by balance factor
   - If magnitude >= cutoff: pass unchanged
5. Perform inverse FFT back to time domain
6. Apply window for overlap-add
7. Mix with dry signal based on dry/wet parameter

### Performance Characteristics
- **CPU Usage**: Efficient FFT implementation
- **Latency**: Fixed at FFT size (1024 samples)
- **Memory**: ~8KB per channel for buffers
- **Thread Safety**: All parameters are thread-safe via atomic operations

## Code Quality

- **Testing**: Comprehensive unit tests with 100% pass rate
- **Documentation**: Full technical and user documentation
- **Architecture**: Clean separation of DSP and UI code
- **Standards**: C++20, JUCE best practices

## Future Enhancements (Optional)

Potential improvements for future versions:
- Variable FFT size for latency/quality tradeoff
- Frequency-dependent threshold curves
- Visual spectrum analyzer
- Preset management system
- Additional windowing functions
- Multi-band processing

## License

See LICENSE file in the repository root.

## Credits

- Built with [JUCE Framework](https://juce.com/)
- Based on [Pamplejuce Template](https://github.com/sudara/pamplejuce)
- FFT implementation uses JUCE's dsp::FFT module
