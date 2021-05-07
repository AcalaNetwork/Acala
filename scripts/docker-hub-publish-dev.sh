#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)

docker build -f scripts/Dockerfile . -t acala/acala-node:$VERSION --no-cache --build-arg GIT_COMMIT=${VERSION} --build-arg CARGO_BUILD_ARGS=--features=with-ethereum-compatibility
docker push acala/acala-node:$VERSION
