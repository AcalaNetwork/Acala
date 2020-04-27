#!/usr/bin/env bash

VERSION=$(git rev-parse --short HEAD)

docker build . -t acala/acala-node:$VERSION --no-cache
docker push acala/acala-node:$VERSION
