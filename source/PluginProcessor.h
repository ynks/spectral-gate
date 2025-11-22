#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_dsp/juce_dsp.h>

#if (MSVC)
#include "ipps.h"
#endif

class PluginProcessor : public juce::AudioProcessor
{
public:
    PluginProcessor();
    ~PluginProcessor() override;

    void prepareToPlay (double sampleRate, int samplesPerBlock) override;
    void releaseResources() override;

    bool isBusesLayoutSupported (const BusesLayout& layouts) const override;

    void processBlock (juce::AudioBuffer<float>&, juce::MidiBuffer&) override;

    juce::AudioProcessorEditor* createEditor() override;
    bool hasEditor() const override;

    const juce::String getName() const override;

    bool acceptsMidi() const override;
    bool producesMidi() const override;
    bool isMidiEffect() const override;
    double getTailLengthSeconds() const override;

    int getNumPrograms() override;
    int getCurrentProgram() override;
    void setCurrentProgram (int index) override;
    const juce::String getProgramName (int index) override;
    void changeProgramName (int index, const juce::String& newName) override;

    void getStateInformation (juce::MemoryBlock& destData) override;
    void setStateInformation (const void* data, int sizeInBytes) override;

    // Parameter access for the editor
    juce::AudioProcessorValueTreeState& getParameters() { return parameters; }
    
    // Spectrum data access for visualization
    void getSpectrumData(std::vector<float>& magnitudes, std::vector<bool>& gateStatus);
    int getFFTSize() const { return currentFFTSize; }

private:
    // Parameters
    juce::AudioProcessorValueTreeState parameters;
    std::atomic<float>* cutoffAmplitudeParam = nullptr;
    std::atomic<float>* weakStrongBalanceParam = nullptr;
    std::atomic<float>* dryWetParam = nullptr;
    std::atomic<float>* fftSizeParam = nullptr;

    // FFT processing - now dynamic
    static constexpr int maxFFTOrder = 11;  // 2048 samples
    static constexpr int maxFFTSize = 1 << maxFFTOrder;
    
    int currentFFTOrder = 10;  // Default 1024 samples
    int currentFFTSize = 1024;
    int currentHopSize = 256;  // 75% overlap
    
    std::unique_ptr<juce::dsp::FFT> forwardFFT;
    std::unique_ptr<juce::dsp::WindowingFunction<float>> window;
    
    std::vector<float> fftData;
    std::vector<float> inputFIFO;
    std::vector<float> outputFIFO;
    std::vector<float> outputAccumulator;
    
    int inputFIFOWritePos = 0;
    int outputFIFOReadPos = 0;
    int outputFIFOWritePos = 0;
    
    // Spectrum data for visualization
    std::vector<float> spectrumMagnitudes;
    std::vector<bool> spectrumGateStatus;
    juce::CriticalSection spectrumLock;

    // Helper method to create parameter layout
    static juce::AudioProcessorValueTreeState::ParameterLayout createParameterLayout();
    void processFFTFrame();
    void updateFFTSize();
    int fftSizeToOrder(int size) const;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (PluginProcessor)
};
