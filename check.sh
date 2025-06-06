#!/bin/sh

export RUSTFLAGS="-D warnings"

./format.sh --check
cargo check --all-features
cargo clippy
cargo clippy --all-features --tests
