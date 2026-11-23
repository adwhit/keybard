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
just play --mode play --autopilot  # watch the built-in player go for a full combo
just smoke                         # whole song headlessly, no window/GPU/audio
just snapshot                      # render PNGs offscreen into target/snapshots
```

Built-in songs: `ode-to-joy`, `twinkle`, `scale`. Any Standard MIDI File
works too: with two or more note tracks the first is the right hand, and with a
single track the split is at middle C.

### Modes (Tab cycles)

- **Listen**: the song plays itself, with sine-wave sound.
- **Wait**: the clock stops at each note until you play it (all notes of a chord).
- **Play**: the clock keeps running. Notes you miss by more than 0.2s count as missed.

### Practising one hand

`--hands right` (or `left`), or F2 in-app, makes you responsible for one hand
only. The computer plays the other hand on time, never scores it, and draws its
bars as translucent ghosts. Bests are kept separately per hand setting.

### Scoring (Wait and Play)

Each hit is graded by timing: **Perfect** (within 50ms, 300 points),
**Great** (within 120ms, 200) or **Good** (within 200ms, 100). In Wait mode,
time spent waiting counts as lateness. Every 10 hits in a row raise the
multiplier, up to x4. A miss or a wrong note resets it.

At the end you get a grade (S/A/B/C/D, by accuracy), your points, max combo and
a breakdown, compared against your personal best for that song and mode.
Bests are saved in the user data directory, or a file passed with `--records`.
Headless and snapshot runs never touch your bests.

Feedback while playing: particle bursts and key flashes on hits, floating
PERFECT/GREAT/GOOD/MISS text, a combo counter that brightens as the multiplier
climbs, combo milestones, an early/late readout in ms, and a progress strip
that records every judgement as a coloured tick.

### Controls

| Key | Action |
| --- | --- |
| Space | play / pause (replay when finished) |
| Left / Right | seek ±5s |
| Up / Down or [ / ] | speed ±5% (25%-200%) |
| Tab | cycle mode |
| F1 | show / hide note names on bars |
| F2 | practise both hands / right / left |
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

Correctness is verified through tests, not by opening the window.
`app::build` takes a `Backend`:

- `Backend::Headless` builds the full app with no window, GPU or audio device,
  using a single-threaded schedule. `tests/integration/app.rs` steps it one
  frame at a time with a fixed clock, injects keyboard and MIDI input (or uses
  the autopilot), and checks ECS state: bar and notehead positions, key colours,
  emitted tones, effects, score, results and saved bests.
- `Backend::Offscreen` renders on the GPU into an image (still no window) and
  saves PNGs at chosen song times. It's for reviewing how things look:
  `just snapshot ode-to-joy "--mode play --autopilot --at 9.2"`.

Test speed notes:

- All integration tests live in one binary (`tests/integration/main.rs`),
  because each Bevy-linked test binary costs a link and a load.
- Dependencies build without debug info, and `.cargo/config.toml` caps test
  threads at 6. Past that, the parallel Bevy apps contend and the suite gets
  slower.
- An edit → `cargo test` cycle takes about 4-5s, mostly compiling and linking.
  Bevy's `dynamic_linking` was tried and was slower here.
- Don't alternate Bevy feature sets in one target directory. Each switch
  rebuilds Bevy (minutes).

Layout:

- `src/music.rs`: pitches, pitch classes, clefs, staff positions
- `src/song.rs`: song model and the small text notation used by `src/builtin.rs`
- `src/midi_file.rs`: `.mid` loading with tempo maps
- `src/performance.rs`: game rules (time advance per mode, timing grades, combo)
- `src/records.rs`: personal bests and their TSV file
- `src/layout.rs`: screen geometry (key placement, fall and staff coordinates)
- `src/app/*`: Bevy plugins for transport, input, audio, piano, falling bars,
  notation, HUD, effects, results, autopilot, generated textures and
  offscreen snapshots

## Status

Working: grand-staff notation (clefs, ledger lines, stems, flags, sharps, bar
lines, noteheads coloured by result), rounded falling bars with note names, a
lit keyboard, three play modes, timing grades, combos, results and personal
bests, one-hand practice, hit effects, an autopilot, MIDI file and MIDI device input, and
sine-wave audio.

Not yet done: key signatures and flats (all accidentals are written as
sharps), rests in the notation, beaming, a nicer synth, a song picker UI, and
choosing a MIDI port.
