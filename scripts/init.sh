#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI ] ; then
   rustup toolchain install nightly-2020-06-04
   rustup default nightly-2020-06-04
   rustup update stable
fi

rustup target add wasm32-unknown-unknown --toolchain nightly
