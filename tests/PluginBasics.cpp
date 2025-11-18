#include "helpers/test_helpers.h"
#include <PluginProcessor.h>
#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_string.hpp>

TEST_CASE ("one is equal to one", "[dummy]")
{
    REQUIRE (1 == 1);
}

TEST_CASE ("Plugin instance", "[instance]")
{
    PluginProcessor testPlugin;

    SECTION ("name")
    {
        CHECK_THAT (testPlugin.getName().toStdString(),
            Catch::Matchers::Equals ("Pamplejuce Demo"));
    }
}

TEST_CASE ("Spectral Gate Parameters", "[parameters]")
{
    PluginProcessor testPlugin;

    SECTION ("has cutoff parameter")
    {
        auto* param = testPlugin.getParameters().getParameter("cutoff");
        REQUIRE(param != nullptr);
    }

    SECTION ("has balance parameter")
    {
        auto* param = testPlugin.getParameters().getParameter("balance");
        REQUIRE(param != nullptr);
    }

    SECTION ("has dry/wet parameter")
    {
        auto* param = testPlugin.getParameters().getParameter("drywet");
        REQUIRE(param != nullptr);
    }
}

TEST_CASE ("Audio Processing", "[processing]")
{
    PluginProcessor testPlugin;
    
    SECTION ("processes audio without crashing")
    {
        testPlugin.prepareToPlay(44100.0, 512);
        
        juce::AudioBuffer<float> buffer(2, 512);
        juce::MidiBuffer midiBuffer;
        
        // Fill buffer with test signal
        for (int channel = 0; channel < buffer.getNumChannels(); ++channel)
        {
            auto* channelData = buffer.getWritePointer(channel);
            for (int sample = 0; sample < buffer.getNumSamples(); ++sample)
            {
                channelData[sample] = std::sin(2.0 * juce::MathConstants<double>::pi * 440.0 * sample / 44100.0);
            }
        }
        
        // Process several buffers to fill the FFT pipeline
        for (int i = 0; i < 5; ++i)
        {
            testPlugin.processBlock(buffer, midiBuffer);
            
            // Refill with test signal for next iteration
            if (i < 4)
            {
                for (int channel = 0; channel < buffer.getNumChannels(); ++channel)
                {
                    auto* channelData = buffer.getWritePointer(channel);
                    for (int sample = 0; sample < buffer.getNumSamples(); ++sample)
                    {
                        channelData[sample] = std::sin(2.0 * juce::MathConstants<double>::pi * 440.0 * sample / 44100.0);
                    }
                }
            }
        }
        
        // Check that output is not all zeros (after pipeline is filled)
        bool hasNonZero = false;
        for (int channel = 0; channel < buffer.getNumChannels(); ++channel)
        {
            auto* channelData = buffer.getReadPointer(channel);
            for (int sample = 0; sample < buffer.getNumSamples(); ++sample)
            {
                if (std::abs(channelData[sample]) > 0.0001f)
                {
                    hasNonZero = true;
                    break;
                }
            }
        }
        REQUIRE(hasNonZero);
    }
}


#ifdef PAMPLEJUCE_IPP
    #include <ipp.h>

TEST_CASE ("IPP version", "[ipp]")
{
    CHECK_THAT (ippsGetLibVersion()->Version, Catch::Matchers::Equals ("2022.2.0 (r0x42db1a66)"));
}
#endif
