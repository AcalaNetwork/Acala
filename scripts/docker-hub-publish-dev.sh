#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)

docker build -f scripts/Dockerfile-dev . -t acala/acala-node:$VERSION --no-cache
docker push acala/acala-node:$VERSION
