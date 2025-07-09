WavetableOscillator with the Rodio library
Currently can play multiple notes at once,
but only one wavetable is used.
The wavetable is generated from a sine wave,
and the frequency is hardcoded based on the key.
The wavetable is then played using the Rodio library.

currently keys are hardcoded to
C4, D4, E4, F4
a,  s,  d,  f,

UI is not implemented yet, but will be added with
ratatui for a terminal UI with retro style.

Future plans:
- 3 oscillators with multiple voices (unison)
- More waveforms (square, saw, triangle)
- Each oscillator can be changed independently
- Basic oscillator options like volume, pan, pitch, waveform etc.
- More advanced options like ADSR, LFO,

- UI with ratatui with knobs and sliders
- Waveform visualization

For now this is the first roadmap to get the basic
functionality working.

If i reach this goal, i will most likely add other
features like effects, waveform editing, arpeggiator,
sequencer, etc.
