#!/usr/bin/env bash

set -e

PROFILE=${PROFILE:-"default"}
TOOLCHAIN=${TOOLCHAIN:-"nightly-2022-08-05"}

echo "Install rust with profile '$PROFILE' and toolchain '$TOOLCHAIN'"

curl https://sh.rustup.rs -sSf | sh -s -- --profile=$PROFILE --default-toolchain=$TOOLCHAIN --target=wasm32-unknown-unknown -y

echo "$HOME/.cargo/bin" >> $GITHUB_PATH
