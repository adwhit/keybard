# keybard

A falling-bars piano trainer in the style of Synthesia, written in Rust with
[Bevy](https://bevyengine.org) 0.20.

```
┌──────────────────────────────────────────────┐
│  𝄞 ═══ grand staff, notes scroll ← to the    │  notation
│  𝄢 ═══ red playhead                          │
├──────────────────────────────────────────────┤
│      ▮        ▮                              │
│   ▮  ▮    ▮   ▮     falling bars             │  falling area
│   ▮       ▮         (green = right hand,     │
│                      blue = left hand)       │
├━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┤  hit line
│ ▌▌ ▌▌▌ ▌▌ ▌▌▌ ▌▌ ▌▌▌   piano keys            │  keyboard
└──────────────────────────────────────────────┘
```

## Running

Needs a recent stable Rust toolchain and [`just`](https://just.systems)
(pinned in `mise.toml`).

```bash
just play                          # Ode to Joy, Listen mode
just play --song twinkle --mode wait
just play-midi path/to/song.mid
just smoke                         # whole song headlessly, no window/GPU/audio
```

Built-in songs: `ode-to-joy`, `twinkle`, `scale`. Any Standard MIDI File
works too: with two or more note tracks the first is the right hand, and with a
single track the split is at middle C.

### Modes (Tab cycles)

- **Listen**: the song plays itself, with sine-wave sound.
- **Wait**: the clock stops at each note until you play it (all notes of a chord).
- **Play**: the clock keeps running. Notes you miss by more than 0.2s count as missed.

### Controls

| Key | Action |
| --- | --- |
| Space | play / pause (replay when finished) |
| ← / → | seek ±5s |
| ↑ / ↓ or [ / ] | speed ±5% (25%–200%) |
| Tab | cycle mode |
| Backspace | restart |
| Esc | quit |
| Z S X D C V … | play the lower octave (C3 by default) |
| Q 2 W 3 E R … | play the upper octave (C4 by default) |
| - / = | shift the computer-keyboard octave |

A connected MIDI keyboard is picked up automatically (the first input port).
Use `--no-midi` to disable it.

## Development

```bash
just check   # fmt-check + clippy -D warnings + tests
just test
```

Everything is verified through tests, not by opening the window.
`app::build` takes a `Backend`. `Backend::Headless` builds the full app with no
window, GPU or audio device, and `tests/app.rs` steps it one frame at a time
with a fixed clock. The tests inject keyboard and MIDI input, then check the
ECS state: bar positions, notehead positions, key colours, emitted tones, score
and transport.

Layout:

- `src/music.rs`: pitches, pitch classes, clefs, staff positions
- `src/song.rs`: song model and the small text notation used by `src/builtin.rs`
- `src/midi_file.rs`: `.mid` loading with tempo maps
- `src/performance.rs`: game rules (time advance per mode, hit/miss judging)
- `src/layout.rs`: screen geometry (key placement, fall and staff coordinates)
- `src/app/*`: Bevy plugins for transport, input, audio, piano, falling bars,
  notation and HUD

## Status

Working: grand-staff notation (clefs, ledger lines, stems, flags, sharps, bar
lines), falling bars, a lit keyboard, three play modes, scoring, MIDI file and
MIDI device input, and sine-wave audio.

Not yet done: key signatures and flats (all accidentals are written as
sharps), rests in the notation, beaming, a nicer synth, a song picker UI, and
choosing a MIDI port.
