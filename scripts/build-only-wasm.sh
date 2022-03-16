#!/usr/bin/env sh

# Script for building only the WASM binary of the given project.

set -xe

PROJECT_ROOT=`git rev-parse --show-toplevel`

if [ "$#" -lt 1 ]; then
  echo "You need to pass the name of the crate you want to compile!"
  exit 1
fi

export WASM_TARGET_DIRECTORY=$(pwd)

cargo build $*
