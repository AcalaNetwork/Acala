#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

rustup default nightly-2021-03-01

rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-01
