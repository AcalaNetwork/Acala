#!/usr/bin/env bash

set -e

declare -a CLEAN_LIST=(
    "acala-service"
    "acala-cli"
    "acala-runtime"
    "e2e-tests"
    "mandala-runtime"
    "karura-runtime"
)

for val in "${CLEAN_LIST[@]}"; do
    echo "cleaning $val"
    cargo clean -p $val
done
