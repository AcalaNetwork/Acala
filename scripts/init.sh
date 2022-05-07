#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

rustup default nightly-2022-02-19

rustup target add wasm32-unknown-unknown --toolchain nightly-2022-02-19
