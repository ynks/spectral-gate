#include "PluginProcessor.h"
#include "PluginEditor.h"

//==============================================================================
juce::AudioProcessorValueTreeState::ParameterLayout PluginProcessor::createParameterLayout()
{
    juce::AudioProcessorValueTreeState::ParameterLayout layout;

    layout.add(std::make_unique<juce::AudioParameterFloat>(
        "cutoff",
        "Cutoff Amplitude",
        juce::NormalisableRange<float>(-60.0f, 0.0f, 0.1f),
        -30.0f,
        juce::String(),
        juce::AudioProcessorParameter::genericParameter,
        [](float value, int) { return juce::String(value, 1) + " dB"; }
    ));

    layout.add(std::make_unique<juce::AudioParameterFloat>(
        "balance",
        "Weak/Strong Balance",
        juce::NormalisableRange<float>(0.0f, 1.0f, 0.01f),
        0.5f,
        juce::String(),
        juce::AudioProcessorParameter::genericParameter,
        [](float value, int) { return juce::String(static_cast<int>(value * 100.0f)) + "%"; }
    ));

    layout.add(std::make_unique<juce::AudioParameterFloat>(
        "drywet",
        "Dry/Wet",
        juce::NormalisableRange<float>(0.0f, 1.0f, 0.01f),
        1.0f,
        juce::String(),
        juce::AudioProcessorParameter::genericParameter,
        [](float value, int) { return juce::String(static_cast<int>(value * 100.0f)) + "%"; }
    ));

    return layout;
}

//==============================================================================
PluginProcessor::PluginProcessor()
     : AudioProcessor (BusesProperties()
                     #if ! JucePlugin_IsMidiEffect
                      #if ! JucePlugin_IsSynth
                       .withInput  ("Input",  juce::AudioChannelSet::stereo(), true)
                      #endif
                       .withOutput ("Output", juce::AudioChannelSet::stereo(), true)
                     #endif
                       ),
       parameters(*this, nullptr, "Parameters", createParameterLayout()),
       forwardFFT(fftOrder),
       window(fftSize, juce::dsp::WindowingFunction<float>::hann)
{
    cutoffAmplitudeParam = parameters.getRawParameterValue("cutoff");
    weakStrongBalanceParam = parameters.getRawParameterValue("balance");
    dryWetParam = parameters.getRawParameterValue("drywet");

    fftData.fill(0.0f);
    inputFIFO.fill(0.0f);
    outputFIFO.fill(0.0f);
    outputAccumulator.fill(0.0f);
    
    // Initialize spectrum data
    spectrumMagnitudes.resize(fftSize / 2, 0.0f);
    spectrumGateStatus.resize(fftSize / 2, false);
}

PluginProcessor::~PluginProcessor()
{
}

//==============================================================================
const juce::String PluginProcessor::getName() const
{
    return JucePlugin_Name;
}

bool PluginProcessor::acceptsMidi() const
{
   #if JucePlugin_WantsMidiInput
    return true;
   #else
    return false;
   #endif
}

bool PluginProcessor::producesMidi() const
{
   #if JucePlugin_ProducesMidiOutput
    return true;
   #else
    return false;
   #endif
}

bool PluginProcessor::isMidiEffect() const
{
   #if JucePlugin_IsMidiEffect
    return true;
   #else
    return false;
   #endif
}

double PluginProcessor::getTailLengthSeconds() const
{
    return 0.0;
}

int PluginProcessor::getNumPrograms()
{
    return 1;   // NB: some hosts don't cope very well if you tell them there are 0 programs,
                // so this should be at least 1, even if you're not really implementing programs.
}

int PluginProcessor::getCurrentProgram()
{
    return 0;
}

void PluginProcessor::setCurrentProgram (int index)
{
    juce::ignoreUnused (index);
}

const juce::String PluginProcessor::getProgramName (int index)
{
    juce::ignoreUnused (index);
    return {};
}

void PluginProcessor::changeProgramName (int index, const juce::String& newName)
{
    juce::ignoreUnused (index, newName);
}

//==============================================================================
void PluginProcessor::prepareToPlay (double sampleRate, int samplesPerBlock)
{
    juce::ignoreUnused (sampleRate, samplesPerBlock);
    
    // Reset buffers
    fftData.fill(0.0f);
    inputFIFO.fill(0.0f);
    outputFIFO.fill(0.0f);
    outputAccumulator.fill(0.0f);
    inputFIFOWritePos = 0;
    outputFIFOReadPos = 0;
    outputFIFOWritePos = 0;
}

void PluginProcessor::releaseResources()
{
    // When playback stops, you can use this as an opportunity to free up any
    // spare memory, etc.
}

bool PluginProcessor::isBusesLayoutSupported (const BusesLayout& layouts) const
{
  #if JucePlugin_IsMidiEffect
    juce::ignoreUnused (layouts);
    return true;
  #else
    // This is the place where you check if the layout is supported.
    // In this template code we only support mono or stereo.
    if (layouts.getMainOutputChannelSet() != juce::AudioChannelSet::mono()
     && layouts.getMainOutputChannelSet() != juce::AudioChannelSet::stereo())
        return false;

    // This checks if the input layout matches the output layout
   #if ! JucePlugin_IsSynth
    if (layouts.getMainOutputChannelSet() != layouts.getMainInputChannelSet())
        return false;
   #endif

    return true;
  #endif
}

void PluginProcessor::processFFTFrame()
{
    // Get parameter values
    const float cutoffDB = cutoffAmplitudeParam->load();
    const float cutoffLinear = juce::Decibels::decibelsToGain(cutoffDB);
    const float balance = weakStrongBalanceParam->load();
    
    // Copy input to FFT buffer
    for (int i = 0; i < fftSize; ++i)
    {
        fftData[i] = inputFIFO[i];
        fftData[fftSize + i] = 0.0f;
    }
    
    // Apply window
    window.multiplyWithWindowingTable(fftData.data(), fftSize);
    
    // Perform forward FFT
    forwardFFT.performRealOnlyForwardTransform(fftData.data(), true);
    
    // Apply spectral gate
    // FFT output is in format: [real0, real1, ..., realN/2, imag1, ..., imagN/2-1]
    {
        juce::ScopedLock lock(spectrumLock);
        
        for (int i = 0; i < fftSize; i += 2)
        {
            float real = fftData[i];
            float imag = fftData[i + 1];
            float magnitude = std::sqrt(real * real + imag * imag);
            
            // Store magnitude for visualization
            int bin = i / 2;
            if (bin < static_cast<int>(spectrumMagnitudes.size()))
            {
                spectrumMagnitudes[bin] = magnitude;
                spectrumGateStatus[bin] = (magnitude >= cutoffLinear);
            }
            
            if (magnitude < cutoffLinear)
            {
                // Below threshold - attenuate based on balance
                // balance = 0: full attenuation (strong gate)
                // balance = 1: no attenuation (weak gate)
                float gain = balance;
                fftData[i] *= gain;
                fftData[i + 1] *= gain;
            }
        }
    }
    
    // Perform inverse FFT
    forwardFFT.performRealOnlyInverseTransform(fftData.data());
    
    // Apply window and add to output accumulator (overlap-add)
    window.multiplyWithWindowingTable(fftData.data(), fftSize);
    
    for (int i = 0; i < fftSize; ++i)
    {
        outputAccumulator[i] += fftData[i];
    }
    
    // Copy first hop to output FIFO
    for (int i = 0; i < hopSize; ++i)
    {
        outputFIFO[outputFIFOWritePos] = outputAccumulator[i];
        outputFIFOWritePos = (outputFIFOWritePos + 1) % fftSize;
    }
    
    // Shift accumulator
    for (int i = 0; i < fftSize - hopSize; ++i)
    {
        outputAccumulator[i] = outputAccumulator[i + hopSize];
    }
    for (int i = fftSize - hopSize; i < fftSize; ++i)
    {
        outputAccumulator[i] = 0.0f;
    }
    
    // Shift input FIFO
    for (int i = 0; i < fftSize - hopSize; ++i)
    {
        inputFIFO[i] = inputFIFO[i + hopSize];
    }
    for (int i = fftSize - hopSize; i < fftSize; ++i)
    {
        inputFIFO[i] = 0.0f;
    }
    
    inputFIFOWritePos = fftSize - hopSize;
}

void PluginProcessor::processBlock (juce::AudioBuffer<float>& buffer,
                                              juce::MidiBuffer& midiMessages)
{
    juce::ignoreUnused (midiMessages);

    juce::ScopedNoDenormals noDenormals;
    auto totalNumInputChannels  = getTotalNumInputChannels();
    auto totalNumOutputChannels = getTotalNumOutputChannels();

    // Clear unused output channels
    for (auto i = totalNumInputChannels; i < totalNumOutputChannels; ++i)
        buffer.clear (i, 0, buffer.getNumSamples());

    // Get dry/wet parameter
    const float dryWet = dryWetParam->load();

    const int numSamples = buffer.getNumSamples();
    
    // Process each channel
    for (int channel = 0; channel < totalNumInputChannels; ++channel)
    {
        auto* channelData = buffer.getWritePointer (channel);
        
        // Store dry signal for mixing later
        std::vector<float> drySignal(channelData, channelData + numSamples);
        
        // Process each sample
        for (int sample = 0; sample < numSamples; ++sample)
        {
            // Add sample to input FIFO
            inputFIFO[inputFIFOWritePos] = channelData[sample];
            inputFIFOWritePos++;
            
            // When we have a full hop, process FFT
            if (inputFIFOWritePos >= fftSize)
            {
                processFFTFrame();
            }
            
            // Get output from output FIFO
            channelData[sample] = outputFIFO[outputFIFOReadPos];
            outputFIFO[outputFIFOReadPos] = 0.0f;
            outputFIFOReadPos = (outputFIFOReadPos + 1) % fftSize;
        }
        
        // Apply dry/wet mix
        for (int i = 0; i < numSamples; ++i)
        {
            channelData[i] = drySignal[i] * (1.0f - dryWet) + channelData[i] * dryWet;
        }
    }
}

//==============================================================================
bool PluginProcessor::hasEditor() const
{
    return true; // (change this to false if you choose to not supply an editor)
}

juce::AudioProcessorEditor* PluginProcessor::createEditor()
{
    return new PluginEditor (*this);
}

//==============================================================================
void PluginProcessor::getStateInformation (juce::MemoryBlock& destData)
{
    auto state = parameters.copyState();
    std::unique_ptr<juce::XmlElement> xml (state.createXml());
    copyXmlToBinary (*xml, destData);
}

void PluginProcessor::setStateInformation (const void* data, int sizeInBytes)
{
    std::unique_ptr<juce::XmlElement> xmlState (getXmlFromBinary (data, sizeInBytes));
    
    if (xmlState.get() != nullptr)
        if (xmlState->hasTagName (parameters.state.getType()))
            parameters.replaceState (juce::ValueTree::fromXml (*xmlState));
}

void PluginProcessor::getSpectrumData(std::vector<float>& magnitudes, std::vector<bool>& gateStatus)
{
    juce::ScopedLock lock(spectrumLock);
    magnitudes = spectrumMagnitudes;
    gateStatus = spectrumGateStatus;
}

//==============================================================================
// This creates new instances of the plugin..
juce::AudioProcessor* JUCE_CALLTYPE createPluginFilter()
{
    return new PluginProcessor();
}
