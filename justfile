alias b := build
alias c := check
alias f := format
alias r := run

_default:
    @just --list

# Build `smaug` in release mode
build:
    cargo build --release

# Check code: formatting, compilation and linting
check:
   cargo +nightly fmt --all -- --check
   cargo +nightly clippy -- -D warnings
   cargo check

# Format code
format:
   cargo +nightly fmt

# Run smaug
run:
    cargo run -r -- -c config.toml
