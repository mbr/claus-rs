#!/bin/sh

set -e

export RUSTFLAGS="-D warnings"

./format.sh --check
cargo check --all-features
cargo clippy
cargo clippy --all-features --tests
cargo clippy --example assistant --features=reqwest-blocking
cargo clippy --example simple_chat --features=reqwest-blocking
cargo clippy --example streaming --features=reqwest
