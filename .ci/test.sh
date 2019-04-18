#!/bin/sh

set -x

# Handy when debugging problems.
cargo --version
rustc --version

FEATURES=""

if [ "$TRAVIS_OS_NAME" = "osx" ]; then
	FEATURES="$FEATURES disable_test_deadline"
fi

cargo test --verbose --features "$FEATURES"
