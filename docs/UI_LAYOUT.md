# Spectral Gate UI Layout

```
┌──────────────────────────────────────────────────────────┐
│                                                          │
│                   Spectral Gate                         │
│                                                          │
├──────────────────────────────────────────────────────────┤
│                                                          │
│   Cutoff Amplitude    Weak/Strong Balance    Dry/Wet    │
│                                                          │
│        ╭───╮              ╭───╮              ╭───╮      │
│       ╱     ╲            ╱     ╲            ╱     ╲     │
│      │   ◯   │          │   ◯   │          │   ◯   │    │
│       ╲     ╱            ╲     ╱            ╲     ╱     │
│        ╰───╯              ╰───╯              ╰───╯      │
│                                                          │
│      -30.0 dB             50%                100%        │
│                                                          │
│                                                          │
│              [ Inspect the UI ]                          │
│                                                          │
└──────────────────────────────────────────────────────────┘

Dimensions: 600 x 400 pixels

Control Details:
- Three rotary knobs with text display below
- Dark background (RGB: 42, 42, 42)
- White text and labels
- Knobs respond to vertical drag for value adjustment
- Text boxes show current parameter values with units
```

## Parameter Ranges

| Parameter          | Range        | Default | Format  |
|--------------------|--------------|---------|---------|
| Cutoff Amplitude   | -60 to 0 dB  | -30 dB  | X.X dB  |
| Weak/Strong Balance| 0 to 100%    | 50%     | XX%     |
| Dry/Wet            | 0 to 100%    | 100%    | XX%     |

## UI Features

- **Real-time parameter updates**: Values update immediately as you adjust knobs
- **Automatable**: All parameters can be automated in your DAW
- **Preset support**: Plugin state is saved/loaded with your project
- **Inspector tool**: Debug button opens Melatonin Inspector for UI development
