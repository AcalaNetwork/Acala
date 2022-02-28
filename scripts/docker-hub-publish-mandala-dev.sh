#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)
NODE_NAME=acala/mandala-node
BUILD_ARGS="build-mandala-internal-release"

docker build -f scripts/Dockerfile . -t $NODE_NAME:$VERSION --build-arg GIT_COMMIT=${VERSION} --build-arg BUILD_ARGS="$BUILD_ARGS"
docker push $NODE_NAME:$VERSION
