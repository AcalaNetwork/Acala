#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)

docker build -f scripts/Dockerfile-parachain . -t acala/acala-node:pc-$VERSION --no-cache
docker push acala/acala-node:pc-$VERSION
