#!/bin/sh

set -e

cargo build
cargo build --example assistant --features=reqwest-blocking
cargo build --example simple_chat --features=reqwest-blocking
cargo build --example streaming --features=reqwest
