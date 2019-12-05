#!/usr/bin/env bash

set -e

cargo clean
WASM_BUILD_TYPE=release cargo run -- build-spec --chain alpha-latest > ./resources/alpha.json
WASM_BUILD_TYPE=release cargo run -- build-spec --chain ./resources/alpha.json --raw > ./resources/alpha-dist.json
