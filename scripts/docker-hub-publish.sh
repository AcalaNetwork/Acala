#!/usr/bin/env bash

set -e

VERSION=$1

if [[ -z "$1" ]] ; then
    echo "Usage: ./scripts/docker-hub-publish.sh VERSION"
    exit 1
fi

docker build -f scripts/Dockerfile-dev . -t acala/acala-node:$1 -t acala/acala-node:latest --build-arg GIT_COMMIT=${VERSION}
docker push acala/acala-node:$1
docker push acala/acala-node:latest
