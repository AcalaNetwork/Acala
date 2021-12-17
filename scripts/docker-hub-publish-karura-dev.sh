#!/usr/bin/env bash

set -e

VERSION=2.0.0
NODE_NAME=ukby1234/karura
BUILD_ARGS="--features with-karura-runtime"

docker build -f scripts/Dockerfile . -t $NODE_NAME:$VERSION --build-arg GIT_COMMIT=${VERSION} --build-arg BUILD_ARGS="$BUILD_ARGS"
docker push $NODE_NAME:$VERSION
