#!/bin/bash
set -e

cd "$(dirname "$0")"

VERSION=$(node -p "require('./package.json').version")
echo "Publishing db6nodejs v$VERSION to npm..."

# Clean
rm -rf dist/ build/

# Publish
npm publish

echo "Published db6nodejs v$VERSION to npm!"