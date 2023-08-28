set shell := ["powershell.exe", "-c"]

default: run

run:
    $env:RUST_LOG="debug"; $env:RUST_BACKTRACE=1; cargo run

build:
    cargo build
