#!/bin/bash
# Script to update AUR package files with release checksums
# Usage: ./update-aur.sh <version>

set -e

VERSION=${1:-$(cat ../Cargo.toml | grep "^version" | sed 's/version = "\(.*\)"/\1/')}

echo "Updating AUR package for version $VERSION"

# Download checksums from GitHub release
REPO="virviil/oci2git"
RELEASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"

# Function to get SHA256 from GitHub release
get_sha256() {
  local filename=$1
  curl -sL "${RELEASE_URL}/${filename}.sha256" | awk '{print $1}'
}

echo "Downloading checksums..."
SHA256_LINUX_X86_64=$(get_sha256 "oci2git-linux-x86_64.tar.gz")
SHA256_LINUX_AARCH64=$(get_sha256 "oci2git-linux-aarch64.tar.gz")

echo "Linux x86_64: $SHA256_LINUX_X86_64"
echo "Linux aarch64: $SHA256_LINUX_AARCH64"

# Update PKGBUILD
PKGBUILD_FILE="PKGBUILD"

cp "$PKGBUILD_FILE" "${PKGBUILD_FILE}.bak"

# Update version
sed -i.tmp "s/pkgver=.*/pkgver=${VERSION}/" "$PKGBUILD_FILE"

# Update SHA256 checksums
sed -i.tmp "s/sha256sums_x86_64=.*/sha256sums_x86_64=('${SHA256_LINUX_X86_64}')/" "$PKGBUILD_FILE"
sed -i.tmp "s/sha256sums_aarch64=.*/sha256sums_aarch64=('${SHA256_LINUX_AARCH64}')/" "$PKGBUILD_FILE"

rm -f "${PKGBUILD_FILE}.tmp"

# Generate .SRCINFO using makepkg
if command -v makepkg &> /dev/null; then
  echo "Generating .SRCINFO..."
  makepkg --printsrcinfo > .SRCINFO
else
  echo "Warning: makepkg not found. Please generate .SRCINFO manually with: makepkg --printsrcinfo > .SRCINFO"

  # Manual update of .SRCINFO if makepkg is not available
  sed -i.tmp "s/pkgver = .*/pkgver = ${VERSION}/" ".SRCINFO"
  sed -i.tmp "s|source_x86_64 = .*|source_x86_64 = https://github.com/${REPO}/releases/download/v${VERSION}/oci2git-linux-x86_64.tar.gz|" ".SRCINFO"
  sed -i.tmp "s|source_aarch64 = .*|source_aarch64 = https://github.com/${REPO}/releases/download/v${VERSION}/oci2git-linux-aarch64.tar.gz|" ".SRCINFO"
  sed -i.tmp "s/sha256sums_x86_64 = .*/sha256sums_x86_64 = ${SHA256_LINUX_X86_64}/" ".SRCINFO"
  sed -i.tmp "s/sha256sums_aarch64 = .*/sha256sums_aarch64 = ${SHA256_LINUX_AARCH64}/" ".SRCINFO"
  rm -f ".SRCINFO.tmp"
fi

echo "AUR package files updated successfully!"
echo ""
echo "Next steps to publish to AUR:"
echo "  1. Clone the AUR repository: git clone ssh://aur@aur.archlinux.org/oci2git-bin.git"
echo "  2. Copy PKGBUILD and .SRCINFO to the cloned repository"
echo "  3. Test the build: makepkg -si"
echo "  4. Commit and push: git add . && git commit -m 'Update to v${VERSION}' && git push"
echo ""
echo "Users can then install with:"
echo "  yay -S oci2git-bin"
echo "  # or"
echo "  paru -S oci2git-bin"
