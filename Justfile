# List recipes
default:
    @just --list

# Run everything CI would: format check, lints, tests
check: fmt-check lint loc test

# Run all unit and integration tests (headless; no window opens)
test *ARGS:
    cargo test {{ARGS}}

# Fail if any Rust file reaches 1000 lines (AGENTS.md: keep files small)
loc:
    @! find src tests -name '*.rs' -exec wc -l {} + | awk '$2 != "total" && $1 >= 1000 {print "too long: " $2 " (" $1 " lines)"; found=1} END {exit !found}'

# Clippy with warnings as errors
lint:
    cargo clippy --all-targets -- -D warnings

# Format the code
fmt:
    cargo fmt

# Verify formatting without changing files
fmt-check:
    cargo fmt --check

# Start the app in release mode with its defaults
run:
    cargo run --release

# Play a song in the window. e.g. `just play --song twinkle --mode wait`
play *ARGS:
    cargo run --release -- {{ARGS}}

# Play a MIDI file in the window
play-midi FILE *ARGS:
    cargo run --release -- {{FILE}} {{ARGS}}

# Run a whole song headlessly (no window, GPU or audio) and exit at the end
smoke SONG="scale":
    cargo run -- --headless --no-midi --song {{SONG}}

# Render offscreen PNGs (no window) at the given song times into target/snapshots
snapshot SONG="ode-to-joy" *TIMES="--at 1 --at 6 --at 12":
    rm -rf target/snapshots && mkdir -p target/snapshots
    cargo run -- --no-midi --song {{SONG}} --snapshot target/snapshots {{TIMES}}

# Render the chord book or its flash cards offscreen into target/snapshots.
# e.g. `just snapshot-book flash-cards --data-dir DIR --player NAME` (needs cleared chords)
snapshot-book SCREEN="chord-book" *ARGS="":
    rm -rf target/snapshots && mkdir -p target/snapshots
    cargo run -- --no-midi --open {{SCREEN}} --snapshot target/snapshots --at=-100 {{ARGS}}

# Render a song through the synth to target/<song>.wav, to listen without the app
listen SONG="ode-to-joy":
    cargo run --release -- --song {{SONG}} --render-wav target/{{SONG}}.wav
