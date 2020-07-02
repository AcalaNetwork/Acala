#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run -- build-spec --chain mandala-latest > ./resources/mandala.json
WASM_BUILD_TYPE=release cargo run -- build-spec --chain ./resources/mandala.json --raw > ./resources/mandala-dist.json
