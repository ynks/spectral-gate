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

    // FFT size parameter (0=64, 1=128, 2=256, 3=512, 4=1024, 5=2048)
    layout.add(std::make_unique<juce::AudioParameterChoice>(
        "fftsize",
        "FFT Size",
        juce::StringArray{"64", "128", "256", "512", "1024", "2048"},
        4  // Default to 1024
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
       parameters(*this, nullptr, "Parameters", createParameterLayout())
{
    cutoffAmplitudeParam = parameters.getRawParameterValue("cutoff");
    weakStrongBalanceParam = parameters.getRawParameterValue("balance");
    dryWetParam = parameters.getRawParameterValue("drywet");
    fftSizeParam = parameters.getRawParameterValue("fftsize");

    // Initialize FFT with default size first
    currentFFTSize = 1024;
    currentFFTOrder = 10;
    currentHopSize = 256;
    
    forwardFFT = std::make_unique<juce::dsp::FFT>(currentFFTOrder);
    window = std::make_unique<juce::dsp::WindowingFunction<float>>(
        currentFFTSize, juce::dsp::WindowingFunction<float>::hann);
    
    fftData.resize(currentFFTSize * 2, 0.0f);
    inputFIFO.resize(currentFFTSize, 0.0f);
    outputFIFO.resize(currentFFTSize, 0.0f);
    outputAccumulator.resize(currentFFTSize, 0.0f);
    
    // Initialize spectrum data
    spectrumMagnitudes.resize(currentFFTSize / 2, 0.0f);
    spectrumGateStatus.resize(currentFFTSize / 2, false);
    
    // Now update to the parameter value (will do nothing if already 1024)
    updateFFTSize();
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
    
    // Update FFT size in case parameter changed
    updateFFTSize();
    
    // Reset buffers
    std::fill(fftData.begin(), fftData.end(), 0.0f);
    std::fill(inputFIFO.begin(), inputFIFO.end(), 0.0f);
    std::fill(outputFIFO.begin(), outputFIFO.end(), 0.0f);
    std::fill(outputAccumulator.begin(), outputAccumulator.end(), 0.0f);
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

int PluginProcessor::fftSizeToOrder(int size) const
{
    switch (size)
    {
        case 64: return 6;
        case 128: return 7;
        case 256: return 8;
        case 512: return 9;
        case 1024: return 10;
        case 2048: return 11;
        default: return 10; // Default to 1024
    }
}

void PluginProcessor::updateFFTSize()
{
    // If parameter not yet initialized, skip update
    if (fftSizeParam == nullptr)
        return;
    
    // Get FFT size from parameter (0=64, 1=128, 2=256, 3=512, 4=1024, 5=2048)
    const int sizeIndex = static_cast<int>(fftSizeParam->load());
    const int sizes[] = {64, 128, 256, 512, 1024, 2048};
    const int newSize = sizes[juce::jlimit(0, 5, sizeIndex)];
    
    if (newSize != currentFFTSize)
    {
        currentFFTSize = newSize;
        currentFFTOrder = fftSizeToOrder(newSize);
        currentHopSize = currentFFTSize / 4; // 75% overlap
        
        // Recreate FFT and window
        forwardFFT = std::make_unique<juce::dsp::FFT>(currentFFTOrder);
        window = std::make_unique<juce::dsp::WindowingFunction<float>>(
            currentFFTSize, juce::dsp::WindowingFunction<float>::hann);
        
        // Resize buffers
        fftData.resize(currentFFTSize * 2, 0.0f);
        inputFIFO.resize(currentFFTSize, 0.0f);
        outputFIFO.resize(currentFFTSize, 0.0f);
        outputAccumulator.resize(currentFFTSize, 0.0f);
        
        // Reset positions
        inputFIFOWritePos = 0;
        outputFIFOReadPos = 0;
        outputFIFOWritePos = 0;
        
        // Update spectrum data sizes
        juce::ScopedLock lock(spectrumLock);
        spectrumMagnitudes.resize(currentFFTSize / 2, 0.0f);
        spectrumGateStatus.resize(currentFFTSize / 2, false);
    }
}

void PluginProcessor::processFFTFrame()
{
    // Get parameter values
    const float cutoffDB = cutoffAmplitudeParam->load();
    const float cutoffLinear = juce::Decibels::decibelsToGain(cutoffDB);
    const float balance = weakStrongBalanceParam->load();
    
    // Copy input to FFT buffer
    for (int i = 0; i < currentFFTSize; ++i)
    {
        fftData[i] = inputFIFO[i];
        fftData[currentFFTSize + i] = 0.0f;
    }
    
    // Apply window
    window->multiplyWithWindowingTable(fftData.data(), currentFFTSize);
    
    // Perform forward FFT
    forwardFFT->performRealOnlyForwardTransform(fftData.data(), true);
    
    // Apply spectral gate
    // FFT output is in format: [real0, real1, ..., realN/2, imag1, ..., imagN/2-1]
    {
        juce::ScopedLock lock(spectrumLock);
        
        for (int i = 0; i < currentFFTSize; i += 2)
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
    forwardFFT->performRealOnlyInverseTransform(fftData.data());
    
    // Normalization factor for the FFT (JUCE doesn't normalize automatically)
    const float normalizationFactor = 1.0f / static_cast<float>(currentFFTSize);
    
    // Add to output accumulator (overlap-add) with normalization
    for (int i = 0; i < currentFFTSize; ++i)
    {
        outputAccumulator[i] += fftData[i] * normalizationFactor;
    }
    
    // Copy first hop to output FIFO
    for (int i = 0; i < currentHopSize; ++i)
    {
        outputFIFO[outputFIFOWritePos] = outputAccumulator[i];
        outputFIFOWritePos = (outputFIFOWritePos + 1) % currentFFTSize;
    }
    
    // Shift accumulator
    for (int i = 0; i < currentFFTSize - currentHopSize; ++i)
    {
        outputAccumulator[i] = outputAccumulator[i + currentHopSize];
    }
    for (int i = currentFFTSize - currentHopSize; i < currentFFTSize; ++i)
    {
        outputAccumulator[i] = 0.0f;
    }
    
    // Shift input FIFO
    for (int i = 0; i < currentFFTSize - currentHopSize; ++i)
    {
        inputFIFO[i] = inputFIFO[i + currentHopSize];
    }
    for (int i = currentFFTSize - currentHopSize; i < currentFFTSize; ++i)
    {
        inputFIFO[i] = 0.0f;
    }
    
    inputFIFOWritePos = currentFFTSize - currentHopSize;
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

    // Check if FFT size changed
    updateFFTSize();

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
            if (inputFIFOWritePos >= currentFFTSize)
            {
                processFFTFrame();
            }
            
            // Get output from output FIFO
            channelData[sample] = outputFIFO[outputFIFOReadPos];
            outputFIFO[outputFIFOReadPos] = 0.0f;
            outputFIFOReadPos = (outputFIFOReadPos + 1) % currentFFTSize;
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
