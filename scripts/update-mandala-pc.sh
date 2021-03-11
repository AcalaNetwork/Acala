#!/usr/bin/env bash

set -e

# cargo clean
WASM_BUILD_TYPE=release cargo run --manifest-path bin/acala/Cargo.toml -- build-spec --chain mandala-latest --raw > ./resources/mandala-pc-dist.json
