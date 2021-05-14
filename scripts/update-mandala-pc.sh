#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run -- build-spec --chain mandala-latest --raw > ./resources/mandala-pc-dist.json
