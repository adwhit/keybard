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
just listen twinkle                # render the song to target/twinkle.wav
```

Built-in songs: `ode-to-joy`, `twinkle`, `scale`, `fur-elise` (the opening),
`minuet` (Minuet in G, simplified) and `jingle-bells` (the chorus). Any Standard MIDI File
works too: with two or more note tracks the first is the right hand, and with a
single track the split is at middle C.

PgUp/PgDn switch songs at any time, cycling through every built-in plus the
MIDI file you opened. During the lead-in and while paused, a song list shows
each one with your best grade and points for the current mode.

### Modes (Tab cycles)

- **Listen**: the song plays itself.
- **Wait**: the clock stops at each note until you play it (all notes of a chord).
- **Play**: the clock keeps running. Notes you miss by more than 0.2s count as missed.

### Sound

Notes play through a small additive synth (`src/synth.rs`) that imitates a
struck string: six slightly stretched harmonics, high ones fading faster,
higher notes decaying sooner, and a soft attack and release so nothing clicks.
`--render-wav FILE` (or `just listen`) mixes a whole song through it offline,
so you can hear it without running the app.

### Count-in and metronome

Every start, restart and replay begins with at least a bar of lead-in. A
metronome clicks the last bar (accent on the first beat) while a big
countdown shows the beats left before the first note. F4 keeps the metronome clicking
on every beat while you play, accented on each bar's downbeat.

### Looping a section

F3 loops the 4 bars you're in, or start with `--loop 9` to drill bars 9-12.
When the clock reaches the end of the section it rewinds to a lead-in before
it, re-arms the notes and counts a pass. The HUD shows the pass number, how
many passes were clean, and the last pass's hits, misses and wrong notes. The
section is highlighted on the progress strip. F3 again stops looping.

In Wait and Play the loop doubles as a speed trainer. Each clean pass (no
misses or wrong notes) raises the speed 5% ("SPEED 75%"), up to full tempo,
and a messy pass holds it. Drilling your weakest bars from the results card
starts at 70%.

### Chord symbols

Lead-sheet chord names (C, G7, Am, Bdim...) scroll above the treble staff
wherever the harmony changes. Each stretch between bar lines and left-hand
chord changes is scored against chord shapes by how long each note sounds, so
passing notes don't flip the chord. A third the arrangement leaves out is
filled in from the key, so a bare G-D in C major reads as G.

### Fingering

Every note gets a recommended finger (1 = thumb ... 5 = pinky), computed for
the tune by `src/fingering.rs`. It runs a dynamic programme over each hand's
notes and chords, using a simple ergonomics model: one finger per white key,
thumb-under and finger-over-thumb crossings at a cost (cheapest with the
middle finger), no other crossings, the same finger on a repeated note, and
thumbs and pinkies kept off black keys. It reproduces textbook fingerings
such as the C major scale (1 2 3 1 2 3 4 5), Ode to Joy in C position and
the Minuet in G opening, and it works on any MIDI file.

Scores can pin a finger with `@`, e.g. `E4@3/2` or `C4@1+E4+G4@5`. The
optimiser keeps pinned notes fixed and plans the fingering around them.

The finger shows as a bold digit on each bar (F1 cycles labels: fingers and
names, fingers, names, off), beside each notehead in the notation, and as a
large digit on a key while its note is about to land or is sounding.

Where the hand has to move (a thumb-under, a finger crossing over the thumb,
or a jump to a new five-finger position), the bar gets an amber strip along
its bottom edge and an amber digit, so you can see shifts coming.

### Sight-reading

F6 (or `--sight-reading`) hides the falling bars and every keyboard cue (hand
colours, approach hints, finger digits on keys), so you play from the staff
alone. Your own presses, hits and misses still show.

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
a breakdown, compared against your personal best for that song and mode. A
strip of cells shows each bar's accuracy from red to green, and the card names
your weakest 4-bar stretch: press F3 there to start drilling it in a loop.
Coaching lines underneath give accuracy for each hand, your timing tendency
("You rush: hits average 34 ms early") and the notes you missed most.
Bests are saved in the user data directory, or a file passed with `--records`.
Headless and snapshot runs never touch your bests.

Reading ahead: your bars brighten over their last half-second of fall, and a
key takes on a pale tint of your hand's colour just before one of your notes
reaches it, so your eyes can move to the keyboard early.

Achievements: thirteen goals unlock as you play. They include a first clear, a
full combo, an S grade, 50 and 100 combos, 99% accuracy, a both-hands full
combo, beating a song above full speed, 3- and 7-day streaks, a 30-minute day
bests on three songs, and sight-reading a song with an A or better. New ones are announced on the results card with a
running count, and they're saved beside your bests (`achievements.txt`). F5
opens a gallery of all thirteen, ticked off as you earn them.

Practice habit: time spent playing (wall-clock, so slowed practice counts in
full) is logged per day next to your bests (`practice.tsv`). At least a minute
on a day extends your day streak. The HUD and results card show the streak and
today's minutes, and crossing the day's minute announces "3-DAY STREAK!".
Days follow your local clock.

Feedback while playing: particle bursts and key flashes on hits, floating
PERFECT/GREAT/GOOD/MISS text, a combo counter that brightens as the multiplier
climbs, "try E4" hints on wrong notes (with the right key flashing), combo
milestones, streak heat (the backdrop warms from violet through
amber to orange as the multiplier climbs, "ON FIRE!" with drifting embers at
x4, and "STREAK LOST" when you break it), an early/late readout in ms, and a progress strip
that records every judgement as a coloured tick.

### Controls

| Key | Action |
| --- | --- |
| Space | play / pause (replay when finished) |
| Left / Right | seek ±5s |
| Up / Down or [ / ] | speed ±5% (25%-200%) |
| Tab | cycle mode |
| F1 | bar labels: fingers + names / fingers / names / off |
| F2 | practise both hands / right / left |
| F3 | loop the current 4 bars / stop looping |
| F4 | metronome on / off |
| F5 | achievements gallery |
| F6 | sight-reading mode (no bars or key cues) |
| PgUp / PgDn | previous / next song |
| Backspace | restart |
| Esc | quit |
| Z S X D C V … | play the lower octave (C3 by default) |
| Q 2 W 3 E R … | play the upper octave (C4 by default) |
| - / = | shift the computer-keyboard octave |

A connected MIDI keyboard is picked up automatically (the first input port).
`--list-midi-ports` shows what's available, `--midi-port yamaha` picks the
first port whose name contains "yamaha", and `--no-midi` disables MIDI. The
HUD shows which device is connected, or why none is.

## Development

```bash
just check   # fmt-check + clippy -D warnings + file-length guard + tests
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
- `src/midi_port.rs`: choosing a MIDI input port
- `src/midi_file.rs`: `.mid` loading with tempo maps
- `src/performance.rs`: game rules (time advance per mode, timing grades, combo)
- `src/records.rs`: personal bests and their TSV file
- `src/achievements.rs`: achievement rules and persistence
- `src/streak.rs`: per-day practice log and day streaks
- `src/chords.rs`: chord naming and the song's chord progression
- `src/key.rs`: key-signature inference and pitch spelling
- `src/notation.rs`: where rests go (gaps split at bar lines into standard values)
- `src/fingering.rs`: automatic fingering (dynamic programming, ergonomic costs)
- `src/synth.rs`: additive struck-string synth (pure sample generation)
- `src/render.rs`: offline song mixdown to WAV
- `src/practice.rs`: loop regions for drilling sections
- `src/layout.rs`: screen geometry (key placement, fall and staff coordinates)
- `src/app/*`: Bevy plugins for transport, input, audio, piano, falling bars,
  notation, HUD, effects, results, autopilot, generated textures and
  offscreen snapshots

## Status

Working: grand-staff notation (clefs, inferred key signatures with sharps or
flats, accidentals only where needed, ledger lines, stems, flags, beams grouped
by the meter, rests, chord symbols,
bar lines, noteheads coloured by result), rounded falling bars with note names, a
lit keyboard, three play modes, timing grades, combos, results and personal
bests, fingering hints, one-hand practice, section looping, a count-in, hit effects, an autopilot, MIDI file and MIDI device input, and a
piano-like additive synth.

Not yet done: sampled piano sound.
