#!/usr/bin/env bash

set -xe

RUSTC_VERSION=`curl -s https://raw.githubusercontent.com/paritytech/srtool/master/RUSTC_VERSION`
PACKAGE=$PACKAGE;
BUILD_OPTS=$BUILD_OPTS;
PROFILE=$PROFILE

docker run --rm -it -e WASM_BUILD_STD=0 -e PROFILE=$PROFILE -e PACKAGE=$PACKAGE -e BUILD_OPTS="$BUILD_OPTS" -v $PWD:/build -v $TMPDIR/cargo:/cargo-home paritytech/srtool:$RUSTC_VERSION $*
