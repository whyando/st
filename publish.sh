#!/bin/bash
set -e

# Configuration
REGISTRY="registry.jpa-dev.whyando.com"
IMAGE_NAME="whyando/spacetraders"
VERSION=$(cargo pkgid | cut -d# -f2 | cut -d: -f2)

echo "Building version: $VERSION"

# Build the image
docker build -t ${REGISTRY}/${IMAGE_NAME}:${VERSION} -t ${REGISTRY}/${IMAGE_NAME}:latest .

# Push the images
docker push ${REGISTRY}/${IMAGE_NAME}:${VERSION}
docker push ${REGISTRY}/${IMAGE_NAME}:latest

echo "Successfully built and pushed ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
