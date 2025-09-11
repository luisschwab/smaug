alias c := check
alias f := format
alias r := run

_default:
    @just --list

# Check code: formatting, compilation and linting
check:
   cargo +nightly fmt --all -- --check
   cargo check

# Format code
format:
   cargo +nightly fmt

# Run smaug
run:
    cargo run -r -- -c config.toml
