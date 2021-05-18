#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run --features with-mandala-runtime --features with-ethereum-compatibility -- build-spec --raw --chain mandala-latest > ./resources/mandala-dist.json
