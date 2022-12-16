#!/usr/bin/env bash

set -e

VERSION=$(git rev-parse --short HEAD)
NODE_NAME=acala/acala-node-profiling

docker buildx build -f scripts/profiling/Dockerfile . -t $NODE_NAME:$VERSION
# docker push $NODE_NAME:$VERSION
