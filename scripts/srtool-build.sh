#!/usr/bin/env bash

set -xe

RUSTC_VERSION=nightly-2021-06-01;
PACKAGE=$PACKAGE;
BUILD_OPTS=$BUILD_OPTS;

docker run --rm -it -e PACKAGE=$PACKAGE -e BUILD_OPTS="$BUILD_OPTS" -v $PWD:/build -v $TMPDIR/cargo:/cargo-home chevdor/srtool:$RUSTC_VERSION $*
