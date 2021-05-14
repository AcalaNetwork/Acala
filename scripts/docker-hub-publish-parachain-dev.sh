#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)
NODE_NAME=acala/karura-node
BUILD_ARGS="--features with-mandala-runtime"

docker build -f scripts/Dockerfile . -t $NODE_NAME:pc-$VERSION --no-cache --build-arg GIT_COMMIT=${VERSION} --build-arg BUILD_ARGS="$BUILD_ARGS"
docker push $NODE_NAME:pc-$VERSION
