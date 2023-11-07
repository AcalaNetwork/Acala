#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

rustup component add rustfmt --toolchain nightly

rustup show
