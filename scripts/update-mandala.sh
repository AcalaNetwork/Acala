#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run --manifest-path bin/acala-dev/Cargo.toml --features with-ethereum-compatibility -- build-spec --raw --chain mandala-latest > ./resources/mandala-dist.json
