#!/bin/bash
set -e

cd "$(dirname "$0")"

VERSION=$(grep '^version = ' pyproject.toml | sed 's/version = "\(.*\)"/\1/')
echo "Publishing db6py v$VERSION to PyPI..."

# Clean
rm -rf dist/ build/ *.egg-info

# Build
echo "Building..."
uv pip install build
uv run build

# Upload to PyPI
echo "Uploading to PyPI..."
uv pip install twine
uv run twine upload dist/*

echo "Published db6py v$VERSION to PyPI!"