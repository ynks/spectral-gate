#include "PluginEditor.h"

PluginEditor::PluginEditor (PluginProcessor& p)
    : AudioProcessorEditor (&p), processorRef (p)
{
    // Setup cutoff slider
    cutoffSlider.setSliderStyle(juce::Slider::RotaryHorizontalVerticalDrag);
    cutoffSlider.setTextBoxStyle(juce::Slider::TextBoxBelow, false, 80, 20);
    addAndMakeVisible(cutoffSlider);
    cutoffAttachment = std::make_unique<juce::AudioProcessorValueTreeState::SliderAttachment>(
        processorRef.getParameters(), "cutoff", cutoffSlider);
    
    cutoffLabel.setText("Cutoff Amplitude", juce::dontSendNotification);
    cutoffLabel.setJustificationType(juce::Justification::centred);
    addAndMakeVisible(cutoffLabel);
    
    // Setup balance slider
    balanceSlider.setSliderStyle(juce::Slider::RotaryHorizontalVerticalDrag);
    balanceSlider.setTextBoxStyle(juce::Slider::TextBoxBelow, false, 80, 20);
    addAndMakeVisible(balanceSlider);
    balanceAttachment = std::make_unique<juce::AudioProcessorValueTreeState::SliderAttachment>(
        processorRef.getParameters(), "balance", balanceSlider);
    
    balanceLabel.setText("Weak/Strong Balance", juce::dontSendNotification);
    balanceLabel.setJustificationType(juce::Justification::centred);
    addAndMakeVisible(balanceLabel);
    
    // Setup dry/wet slider
    dryWetSlider.setSliderStyle(juce::Slider::RotaryHorizontalVerticalDrag);
    dryWetSlider.setTextBoxStyle(juce::Slider::TextBoxBelow, false, 80, 20);
    addAndMakeVisible(dryWetSlider);
    dryWetAttachment = std::make_unique<juce::AudioProcessorValueTreeState::SliderAttachment>(
        processorRef.getParameters(), "drywet", dryWetSlider);
    
    dryWetLabel.setText("Dry/Wet", juce::dontSendNotification);
    dryWetLabel.setJustificationType(juce::Justification::centred);
    addAndMakeVisible(dryWetLabel);
    
    // Setup inspect button
    addAndMakeVisible (inspectButton);

    // this chunk of code instantiates and opens the melatonin inspector
    inspectButton.onClick = [&] {
        if (!inspector)
        {
            inspector = std::make_unique<melatonin::Inspector> (*this);
            inspector->onClose = [this]() { inspector.reset(); };
        }

        inspector->setVisible (true);
    };

    setSize (600, 400);
}

PluginEditor::~PluginEditor()
{
}

void PluginEditor::paint (juce::Graphics& g)
{
    // Fill background
    g.fillAll (juce::Colour(0xff2a2a2a));
    
    // Draw title
    g.setColour (juce::Colours::white);
    g.setFont (24.0f);
    g.drawText ("Spectral Gate", getLocalBounds().removeFromTop(60), juce::Justification::centred, true);
}

void PluginEditor::resized()
{
    auto area = getLocalBounds();
    
    // Title area
    area.removeFromTop(60);
    
    // Controls area
    auto controlsArea = area.reduced(20);
    
    // Three columns for the three knobs
    const int knobWidth = controlsArea.getWidth() / 3;
    
    // Cutoff column
    auto cutoffArea = controlsArea.removeFromLeft(knobWidth).reduced(10);
    cutoffLabel.setBounds(cutoffArea.removeFromTop(30));
    cutoffSlider.setBounds(cutoffArea.removeFromTop(120));
    
    // Balance column
    auto balanceArea = controlsArea.removeFromLeft(knobWidth).reduced(10);
    balanceLabel.setBounds(balanceArea.removeFromTop(30));
    balanceSlider.setBounds(balanceArea.removeFromTop(120));
    
    // Dry/Wet column
    auto dryWetArea = controlsArea.reduced(10);
    dryWetLabel.setBounds(dryWetArea.removeFromTop(30));
    dryWetSlider.setBounds(dryWetArea.removeFromTop(120));
    
    // Inspect button at bottom
    inspectButton.setBounds(getLocalBounds().removeFromBottom(60).withSizeKeepingCentre(120, 40));
}
