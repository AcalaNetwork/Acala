#!/usr/bin/env sh

# Script for building only the WASM binary of the given project.

set -e

PROJECT_ROOT=`git rev-parse --show-toplevel`

if [ "$#" -lt 1 ]; then
  echo "You need to pass the name of the crate you want to compile!"
  exit 1
fi

if [ -z "$2" ]; then
  export WASM_TARGET_DIRECTORY=$(pwd)
else
  export WASM_TARGET_DIRECTORY=$2
fi

cargo build --manifest-path bin/acala/Cargo.toml --release -p $1 --features with-ethereum-compatibility
