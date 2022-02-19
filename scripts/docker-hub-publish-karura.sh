#!/usr/bin/env bash

set -e

VERSION=$1
NODE_NAME=acala/karura-node
BUILD_ARGS="build-karura-release"

if [[ -z "$1" ]] ; then
    VERSION=$(git rev-parse --short HEAD)
fi

docker build -f scripts/Dockerfile . -t $NODE_NAME:$VERSION -t $NODE_NAME:latest --build-arg GIT_COMMIT=${VERSION} --build-arg BUILD_ARGS="$BUILD_ARGS"
docker push $NODE_NAME:$VERSION
docker push $NODE_NAME:latest
