#!/usr/bin/env bash

set -e

curl https://sh.rustup.rs -sSf | sh -s -- -y

source "$HOME/.cargo/env"

make toolchain

echo "$HOME/.cargo/bin" >> $GITHUB_PATH
