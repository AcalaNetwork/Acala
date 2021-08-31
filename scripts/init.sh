#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

rustup default nightly-2021-06-17

rustup target add wasm32-unknown-unknown --toolchain nightly-2021-06-17
