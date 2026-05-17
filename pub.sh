#!/bin/bash
set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 <new_version>"
    echo "Example: $0 4.14.0"
    exit 1
fi

NEW_VERSION="$1"
NAME="db6"

# 驗證版本格式
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: version must be in format major.minor.patch (e.g. 4.14.0)"
    exit 1
fi

echo "=== Checking current version on crates.io ==="
PUBLISHED=$(curl -s "https://crates.io/api/v1/crates/${NAME}" | grep -o '"max_version":"[^"]*"' | cut -d'"' -f4)

if [ -n "$PUBLISHED" ]; then
    echo "Published version: $PUBLISHED"
    if [ "$NEW_VERSION" = "$PUBLISHED" ]; then
        echo "Error: version $NEW_VERSION is already published"
        exit 1
    fi
    # 比較版本號（用 sort -V）
    HIGHER=$(echo -e "$PUBLISHED\n$NEW_VERSION" | sort -V | tail -1)
    if [ "$HIGHER" != "$NEW_VERSION" ]; then
        echo "Error: $NEW_VERSION is not higher than published $PUBLISHED"
        exit 1
    fi
else
    echo "No published version found (first publish)"
fi

echo "=== Updating Cargo.toml version to $NEW_VERSION ==="
sed -i "" "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

echo "=== Running tests ==="
cargo test

echo "=== Committing to git ==="
git add -A
git commit -m "v${NEW_VERSION}"
git push

echo "=== Publishing to crates.io ==="
cargo publish

echo "=== v${NEW_VERSION} published successfully ==="
