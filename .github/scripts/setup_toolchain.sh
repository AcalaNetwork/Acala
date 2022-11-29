#!/usr/bin/env bash

set -e

curl https://sh.rustup.rs -sSf | sh -s -- -y

source "$HOME/.cargo/env"

rustup default nightly-2022-08-05

rustup target add wasm32-unknown-unknown --toolchain nightly-2022-08-05

echo "$HOME/.cargo/bin" >> $GITHUB_PATH
