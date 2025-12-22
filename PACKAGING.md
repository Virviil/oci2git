# Packaging Guide for oci2git

This document describes how to build and distribute oci2git binaries and packages for various platforms.

## Overview

The project uses GitHub Actions to automatically build releases for:
- **Binary releases**: Linux (x86_64, ARM64), macOS (x86_64, ARM64), Windows (x86_64)
- **Debian packages**: .deb packages for Ubuntu/Debian (amd64, arm64)
- **Homebrew**: macOS and Linux via Homebrew tap
- **AUR**: Arch Linux via the AUR (Arch User Repository)

## Automated Release Process

### Creating a New Release

1. Update the version in `Cargo.toml`:
   ```toml
   version = "0.2.5"
   ```

2. Commit the version change:
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to 0.2.5"
   git push
   ```

3. Create and push a version tag:
   ```bash
   git tag v0.2.5
   git push origin v0.2.5
   ```

4. GitHub Actions will automatically:
   - Build binaries for all platforms
   - Create Debian packages
   - Create a GitHub release
   - Upload all artifacts with checksums

## Homebrew Tap Setup

### One-Time Setup

1. Create a new GitHub repository named `homebrew-oci2git`:
   ```bash
   gh repo create homebrew-oci2git --public
   git clone https://github.com/virviil/homebrew-oci2git.git
   cd homebrew-oci2git
   mkdir -p Formula
   ```

2. Copy the formula template:
   ```bash
   cp ../oci2git/homebrew/oci2git.rb Formula/
   ```

### Updating the Formula for Each Release

After creating a new release, update the Homebrew formula:

```bash
cd oci2git
./homebrew/update-formula.sh 0.2.5
```

This script will:
- Download the checksums from the GitHub release
- Update the formula with the new version and checksums
- Display the updated formula

Then copy to your tap repository:

```bash
cp homebrew/oci2git.rb ../homebrew-oci2git/Formula/
cd ../homebrew-oci2git
git add Formula/oci2git.rb
git commit -m "Update oci2git to 0.2.5"
git push
```

### Installing via Homebrew

Users can then install with:

```bash
brew tap virviil/oci2git
brew install oci2git
```

## Debian Package Distribution

Debian packages are automatically built and uploaded to GitHub releases.

### Manual Installation

Users can download and install the appropriate .deb package:

```bash
# For amd64 (x86_64)
wget https://github.com/virviil/oci2git/releases/download/v0.2.5/oci2git_0.2.5_amd64.deb
sudo dpkg -i oci2git_0.2.5_amd64.deb

# For arm64
wget https://github.com/virviil/oci2git/releases/download/v0.2.5/oci2git_0.2.5_arm64.deb
sudo dpkg -i oci2git_0.2.5_arm64.deb
```

### Setting Up a Debian Repository (Optional)

For easier installation and updates, you can set up a Debian package repository using GitHub Pages or a hosting service. Tools like `aptly` or `reprepro` can help manage the repository.

## AUR (Arch Linux) Setup

### One-Time Setup

1. Create an AUR account at https://aur.archlinux.org/

2. Set up SSH keys for AUR:
   ```bash
   ssh-keygen -t ed25519 -C "your_email@example.com"
   # Add the public key to your AUR account settings
   ```

3. Clone the AUR repository (first time):
   ```bash
   git clone ssh://aur@aur.archlinux.org/oci2git-bin.git
   cd oci2git-bin
   ```

### Updating the AUR Package for Each Release

After creating a new release:

```bash
cd oci2git/aur
./update-aur.sh 0.2.5
```

This script will:
- Download checksums from GitHub release
- Update PKGBUILD with the new version and checksums
- Generate .SRCINFO

Then publish to AUR:

```bash
# Copy files to AUR repository
cp PKGBUILD .SRCINFO ../oci2git-bin/

cd ../oci2git-bin

# Test the build (optional but recommended)
makepkg -si

# Commit and push to AUR
git add PKGBUILD .SRCINFO
git commit -m "Update to 0.2.5"
git push
```

### Installing from AUR

Users can install using an AUR helper:

```bash
yay -S oci2git-bin
# or
paru -S oci2git-bin
```

Or manually:

```bash
git clone https://aur.archlinux.org/oci2git-bin.git
cd oci2git-bin
makepkg -si
```

## Binary Releases

Binary releases are automatically uploaded to GitHub Releases and include:

- `oci2git-linux-x86_64.tar.gz`
- `oci2git-linux-aarch64.tar.gz`
- `oci2git-darwin-x86_64.tar.gz` (macOS Intel)
- `oci2git-darwin-aarch64.tar.gz` (macOS Apple Silicon)
- `oci2git-windows-x86_64.exe.tar.gz`

Each archive includes SHA256 checksums for verification.

### Manual Installation

Users can download and install binaries manually:

```bash
# Linux x86_64 example
wget https://github.com/virviil/oci2git/releases/download/v0.2.5/oci2git-linux-x86_64.tar.gz
tar xzf oci2git-linux-x86_64.tar.gz
sudo mv oci2git-linux-x86_64 /usr/local/bin/oci2git
chmod +x /usr/local/bin/oci2git
```

## Troubleshooting

### GitHub Actions Failures

If the release workflow fails:

1. Check the Actions tab in GitHub for error logs
2. Common issues:
   - Cross-compilation toolchain not installed
   - Permission issues with GITHUB_TOKEN
   - Network issues downloading dependencies

### Homebrew Formula Issues

If the formula doesn't work:

1. Test locally:
   ```bash
   brew install --build-from-source ./Formula/oci2git.rb
   ```

2. Check SHA256 mismatches:
   ```bash
   wget <url>
   shasum -a 256 oci2git-*.tar.gz
   ```

### AUR Package Issues

If the PKGBUILD fails:

1. Test locally:
   ```bash
   makepkg -si
   ```

2. Check for:
   - Incorrect SHA256 sums
   - Network issues downloading sources
   - Missing dependencies

## Release Checklist

- [ ] Update version in Cargo.toml
- [ ] Update CHANGELOG.md (if exists)
- [ ] Commit version changes
- [ ] Create and push git tag (v0.2.5)
- [ ] Wait for GitHub Actions to complete
- [ ] Verify all artifacts in GitHub Release
- [ ] Update Homebrew formula
- [ ] Push Homebrew formula to tap
- [ ] Update AUR package
- [ ] Push AUR package update
- [ ] Test installations on each platform
- [ ] Announce release

## Platform-Specific Notes

### macOS Code Signing

For production releases, consider code signing the macOS binaries:

```bash
codesign --sign "Developer ID Application: Your Name" --timestamp oci2git
```

This requires an Apple Developer account.

### Windows Signing

For production releases, consider signing the Windows executable with a code signing certificate.

### Linux AppImage (Future)

Consider creating AppImage packages for broader Linux compatibility.

## Support

For issues with packaging or distribution, please open an issue at:
https://github.com/virviil/oci2git/issues
