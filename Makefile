build:
	cargo build

ifeq ($(TRAVIS_OS_NAME), osx)
	CRATE_FEATURES="test_extended_time_margin"
else
	CRATE_FEATURES=""
endif

test-ci:
	cargo --version
	rustc --version
	cargo build --verbose
	cargo test --verbose --features "$(CRATE_FEATURES)"

.PHONY: build test-ci
