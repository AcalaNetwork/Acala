#!/usr/bin/env bash

set -xe

RUSTC_VERSION=1.62.0;
PACKAGE=$PACKAGE;
BUILD_OPTS=$BUILD_OPTS;

docker run --rm -it -e PACKAGE=$PACKAGE -e BUILD_OPTS="$BUILD_OPTS" -v $PWD:/build -v $TMPDIR/cargo:/cargo-home paritytech/srtool:$RUSTC_VERSION $*
