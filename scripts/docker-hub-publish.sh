#!/usr/bin/env bash

set -e

VERSION=$1
NODE_NAME=acala/acala-node
BUILD_ARGS="--features with-mandala-runtime --features with-ethereum-compatibility"

if [[ -z "$1" ]] ; then
    echo "Usage: ./scripts/docker-hub-publish.sh VERSION"
    exit 1
fi

docker build -f scripts/Dockerfile . -t $NODE_NAME:$1 -t $NODE_NAME:latest --build-arg GIT_COMMIT=${VERSION} --build-arg BUILD_ARGS="$BUILD_ARGS"
docker push $NODE_NAME:$1
docker push $NODE_NAME:latest
