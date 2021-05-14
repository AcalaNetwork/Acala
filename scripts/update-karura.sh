#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run --features with-karura-runtime -- build-spec --raw --chain karura-latest > ./resources/karura-dist.json
