#!/bin/sh

set -eux

# Handy when debugging problems.
rustup --version
cargo --version
rustc --version

TARGET=$1

rustup target add $TARGET
cargo build --verbose --target $TARGET
