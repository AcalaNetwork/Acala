#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run --features with-acala-runtime --features with-ethereum-compatibility -- build-spec --raw --chain acala-latest > ./resources/acala-dist.json
