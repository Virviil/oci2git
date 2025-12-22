#!/bin/bash
# Script to update Homebrew formula with release checksums
# Usage: ./update-formula.sh <version>

set -e

VERSION=${1:-$(cat Cargo.toml | grep "^version" | sed 's/version = "\(.*\)"/\1/')}

echo "Updating Homebrew formula for version $VERSION"

# Download checksums from GitHub release
REPO="virviil/oci2git"
RELEASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"

# Function to get SHA256 from GitHub release
get_sha256() {
  local filename=$1
  curl -sL "${RELEASE_URL}/${filename}.sha256" | awk '{print $1}'
}

echo "Downloading checksums..."
SHA256_DARWIN_ARM64=$(get_sha256 "oci2git-darwin-aarch64.tar.gz")
SHA256_DARWIN_X86_64=$(get_sha256 "oci2git-darwin-x86_64.tar.gz")
SHA256_LINUX_ARM64=$(get_sha256 "oci2git-linux-aarch64.tar.gz")
SHA256_LINUX_X86_64=$(get_sha256 "oci2git-linux-x86_64.tar.gz")

echo "Darwin ARM64: $SHA256_DARWIN_ARM64"
echo "Darwin x86_64: $SHA256_DARWIN_X86_64"
echo "Linux ARM64: $SHA256_LINUX_ARM64"
echo "Linux x86_64: $SHA256_LINUX_X86_64"

# Update formula file
FORMULA_FILE="homebrew/oci2git.rb"

cp "$FORMULA_FILE" "${FORMULA_FILE}.bak"

# Update version
sed -i.tmp "s/version \".*\"/version \"${VERSION}\"/" "$FORMULA_FILE"

# Update SHA256 checksums
sed -i.tmp "s/REPLACE_WITH_ARM64_MACOS_SHA256/${SHA256_DARWIN_ARM64}/" "$FORMULA_FILE"
sed -i.tmp "s/REPLACE_WITH_X86_64_MACOS_SHA256/${SHA256_DARWIN_X86_64}/" "$FORMULA_FILE"
sed -i.tmp "s/REPLACE_WITH_ARM64_LINUX_SHA256/${SHA256_LINUX_ARM64}/" "$FORMULA_FILE"
sed -i.tmp "s/REPLACE_WITH_X86_64_LINUX_SHA256/${SHA256_LINUX_X86_64}/" "$FORMULA_FILE"

rm -f "${FORMULA_FILE}.tmp"

echo "Formula updated successfully!"
echo "Please review the changes and commit to your homebrew tap repository:"
echo "  1. Create a repository named 'homebrew-oci2git' on GitHub"
echo "  2. Copy the updated formula to Formula/oci2git.rb in that repository"
echo "  3. Commit and push"
echo ""
echo "Users can then install with:"
echo "  brew tap virviil/oci2git"
echo "  brew install oci2git"
