#!/bin/sh

set -e

exec cargo test --all-features
