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

private:
    // Parameters
    juce::AudioProcessorValueTreeState parameters;
    std::atomic<float>* cutoffAmplitudeParam = nullptr;
    std::atomic<float>* weakStrongBalanceParam = nullptr;
    std::atomic<float>* dryWetParam = nullptr;

    // FFT processing
    static constexpr int fftOrder = 10;  // 1024 samples
    static constexpr int fftSize = 1 << fftOrder;
    static constexpr int hopSize = fftSize / 4;  // 75% overlap
    
    juce::dsp::FFT forwardFFT;
    juce::dsp::WindowingFunction<float> window;
    
    std::array<float, fftSize * 2> fftData;
    std::array<float, fftSize> inputFIFO;
    std::array<float, fftSize> outputFIFO;
    std::array<float, fftSize> outputAccumulator;
    
    int inputFIFOWritePos = 0;
    int outputFIFOReadPos = 0;
    int outputFIFOWritePos = 0;

    // Helper method to create parameter layout
    static juce::AudioProcessorValueTreeState::ParameterLayout createParameterLayout();
    void processFFTFrame();

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (PluginProcessor)
};
