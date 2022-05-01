#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)
NODE_NAME=acala/acala-node
BUILD_ARGS="build-acala-internal-release"

docker build -f scripts/Dockerfile . -t $NODE_NAME:$VERSION --build-arg GIT_COMMIT=${VERSION} --build-arg BUILD_ARGS="$BUILD_ARGS" --build-arg PROFILE=release
docker push $NODE_NAME:$VERSION
