#pragma once

#include "PluginProcessor.h"
#include "BinaryData.h"
#include "melatonin_inspector/melatonin_inspector.h"

//==============================================================================
class SpectrumAnalyzer : public juce::Component, private juce::Timer
{
public:
    SpectrumAnalyzer(PluginProcessor& processor) : processorRef(processor)
    {
        startTimerHz(30); // Update 30 times per second
    }
    
    void paint(juce::Graphics& g) override
    {
        g.fillAll(juce::Colour(0xff1a1a1a));
        
        // Get spectrum data from processor
        std::vector<float> magnitudes;
        std::vector<bool> gateStatus;
        processorRef.getSpectrumData(magnitudes, gateStatus);
        
        if (magnitudes.empty())
            return;
        
        const auto bounds = getLocalBounds().toFloat();
        const float width = bounds.getWidth();
        const float height = bounds.getHeight();
        
        // Draw frequency grid lines
        g.setColour(juce::Colour(0xff404040));
        for (int i = 1; i < 4; ++i)
        {
            float y = height * i / 4.0f;
            g.drawLine(0, y, width, y, 1.0f);
        }
        
        // Draw spectrum
        juce::Path spectrumPath;
        const int numBins = static_cast<int>(magnitudes.size());
        const float binWidth = width / static_cast<float>(numBins);
        
        bool pathStarted = false;
        for (int i = 1; i < numBins / 2; ++i) // Only show first half (up to Nyquist)
        {
            const float magnitude = magnitudes[i];
            const float normalizedMag = juce::jlimit(0.0f, 1.0f, magnitude * 100.0f);
            const float x = i * binWidth;
            const float y = height * (1.0f - normalizedMag);
            
            if (!pathStarted)
            {
                spectrumPath.startNewSubPath(x, height);
                pathStarted = true;
            }
            
            spectrumPath.lineTo(x, y);
        }
        
        if (pathStarted)
        {
            spectrumPath.lineTo(width / 2, height);
            spectrumPath.closeSubPath();
            
            // Fill spectrum with gradient
            g.setGradientFill(juce::ColourGradient(
                juce::Colour(0xff00ff00).withAlpha(0.3f), 0, height,
                juce::Colour(0xff00ff00).withAlpha(0.6f), 0, 0,
                false));
            g.fillPath(spectrumPath);
            
            // Draw outline
            g.setColour(juce::Colour(0xff00ff00));
            g.strokePath(spectrumPath, juce::PathStrokeType(2.0f));
        }
        
        // Draw gated regions in red
        g.setColour(juce::Colour(0xffff0000).withAlpha(0.3f));
        for (int i = 1; i < numBins / 2; ++i)
        {
            if (!gateStatus[i])
            {
                const float x = i * binWidth;
                g.fillRect(x, 0.0f, binWidth, height);
            }
        }
        
        // Draw border
        g.setColour(juce::Colour(0xff606060));
        g.drawRect(bounds, 2.0f);
        
        // Draw labels
        g.setColour(juce::Colours::white);
        g.setFont(12.0f);
        auto labelBounds = bounds;
        g.drawText("Frequency Spectrum", labelBounds.removeFromTop(20), juce::Justification::centred);
    }
    
    void timerCallback() override
    {
        repaint();
    }
    
private:
    PluginProcessor& processorRef;
};

//==============================================================================
class PluginEditor : public juce::AudioProcessorEditor
{
public:
    explicit PluginEditor (PluginProcessor&);
    ~PluginEditor() override;

    //==============================================================================
    void paint (juce::Graphics&) override;
    void resized() override;

private:
    // This reference is provided as a quick way for your editor to
    // access the processor object that created it.
    PluginProcessor& processorRef;
    std::unique_ptr<melatonin::Inspector> inspector;
    juce::TextButton inspectButton { "Inspect the UI" };
    
    // Spectrum analyzer
    SpectrumAnalyzer spectrumAnalyzer;
    
    // Parameter controls
    juce::Slider cutoffSlider;
    juce::Label cutoffLabel;
    std::unique_ptr<juce::AudioProcessorValueTreeState::SliderAttachment> cutoffAttachment;
    
    juce::Slider balanceSlider;
    juce::Label balanceLabel;
    std::unique_ptr<juce::AudioProcessorValueTreeState::SliderAttachment> balanceAttachment;
    
    juce::Slider dryWetSlider;
    juce::Label dryWetLabel;
    std::unique_ptr<juce::AudioProcessorValueTreeState::SliderAttachment> dryWetAttachment;
    
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (PluginEditor)
};
