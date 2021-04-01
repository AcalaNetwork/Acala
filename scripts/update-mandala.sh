#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run --manifest-path bin/acala-dev/Cargo.toml --features with-mandala-runtime -- build-spec --raw --chain local > ./resources/mandala-dist.json
