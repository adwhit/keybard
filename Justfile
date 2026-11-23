# List recipes
default:
    @just --list

# Run everything CI would: format check, lints, tests
check: fmt-check lint test

# Run all unit and integration tests (headless; no window opens)
test *ARGS:
    cargo test {{ARGS}}

# Clippy with warnings as errors
lint:
    cargo clippy --all-targets -- -D warnings

# Format the code
fmt:
    cargo fmt

# Verify formatting without changing files
fmt-check:
    cargo fmt --check

# Play a song in the window. e.g. `just play --song twinkle --mode wait`
play *ARGS:
    cargo run --release -- {{ARGS}}

# Play a MIDI file in the window
play-midi FILE *ARGS:
    cargo run --release -- {{FILE}} {{ARGS}}

# Run a whole song headlessly (no window, GPU or audio) and exit at the end
smoke SONG="scale":
    cargo run -- --headless --no-midi --song {{SONG}}
