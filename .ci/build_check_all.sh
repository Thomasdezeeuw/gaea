#!/bin/sh

set -eux

.ci/build_check.sh "aarch64-linux-android"
.ci/build_check.sh "x86_64-apple-darwin"
.ci/build_check.sh "x86_64-unknown-freebsd"
.ci/build_check.sh "x86_64-unknown-linux-gnu"
.ci/build_check.sh "x86_64-unknown-linux-musl"
.ci/build_check.sh "x86_64-unknown-netbsd"
