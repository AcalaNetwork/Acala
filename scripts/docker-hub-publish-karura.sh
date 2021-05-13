#!/usr/bin/env bash

set -e

VERSION=$1
NODE_NAME=acala/karura-node

if [[ -z "$1" ]] ; then
    echo "Usage: ./scripts/docker-hub-publish-karura.sh VERSION"
    exit 1
fi

docker build -f scripts/Dockerfile . -t $NODE_NAME:$1 -t $NODE_NAME:latest --build-arg GIT_COMMIT=${VERSION} --build-arg PROFILE=111
docker push $NODE_NAME:$1
docker push $NODE_NAME:latest
