#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)

docker build -f scripts/Dockerfile-parachain . -t acala/acala-node:pc-$VERSION --no-cache --build-arg GIT_COMMIT=${VERSION}
docker push acala/acala-node:pc-$VERSION
