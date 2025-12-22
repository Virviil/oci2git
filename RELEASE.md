# Release Process Quick Reference

This document provides a quick checklist for creating and publishing a new release of oci2git.

## Pre-Release Checklist

- [ ] All tests are passing (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation is up to date
- [ ] CHANGELOG.md is updated (if exists)

## Creating a Release

### 1. Update Version

Edit `Cargo.toml`:

```toml
version = "0.2.5"
```

### 2. Commit and Tag

```bash
git add Cargo.toml
git commit -m "Bump version to 0.2.5"
git push

git tag v0.2.5
git push origin v0.2.5
```

### 3. Wait for CI

GitHub Actions will automatically:
- Build binaries for all platforms
- Create Debian packages
- Create a GitHub Release
- Upload all artifacts

Monitor progress at: https://github.com/virviil/oci2git/actions

### 4. Update Homebrew Tap

```bash
./homebrew/update-formula.sh 0.2.5

# Copy to tap repository
cp homebrew/oci2git.rb ../homebrew-oci2git/Formula/
cd ../homebrew-oci2git
git add Formula/oci2git.rb
git commit -m "Update oci2git to 0.2.5"
git push
```

### 5. Update AUR Package

```bash
cd aur
./update-aur.sh 0.2.5

# Copy to AUR repository
cp PKGBUILD .SRCINFO ~/oci2git-bin/
cd ~/oci2git-bin
git add PKGBUILD .SRCINFO
git commit -m "Update to 0.2.5"
git push
```

## Verification

After release, verify installations work:

```bash
# Homebrew
brew upgrade oci2git
oci2git --version

# Debian
wget https://github.com/virviil/oci2git/releases/download/v0.2.5/oci2git_0.2.5_amd64.deb
sudo dpkg -i oci2git_0.2.5_amd64.deb
oci2git --version

# AUR
yay -Syu oci2git-bin
oci2git --version
```

## Troubleshooting

### GitHub Actions Failed

1. Check the workflow logs
2. Re-run the failed jobs if it's a transient issue
3. Delete the tag and release if you need to fix code:
   ```bash
   git tag -d v0.2.5
   git push origin :v0.2.5
   # Fix the issue, then repeat steps 1-3
   ```

### Homebrew Formula Not Working

Test locally:
```bash
brew install --build-from-source ./homebrew/oci2git.rb
```

### AUR Package Not Building

Test locally:
```bash
cd aur
makepkg -si
```

## First-Time Setup

### Homebrew Tap

Create the tap repository once:

```bash
gh repo create homebrew-oci2git --public
git clone https://github.com/virviil/homebrew-oci2git.git
cd homebrew-oci2git
mkdir -p Formula
# Then follow step 4 above for each release
```

### AUR

Set up AUR access once:

```bash
# Generate SSH key and add to AUR account
ssh-keygen -t ed25519
# Add public key to https://aur.archlinux.org/account/

# Clone AUR repository
git clone ssh://aur@aur.archlinux.org/oci2git-bin.git
# Then follow step 5 above for each release
```

## Release Artifacts

Each release creates:

- `oci2git-linux-x86_64.tar.gz` + `.sha256`
- `oci2git-linux-aarch64.tar.gz` + `.sha256`
- `oci2git-darwin-x86_64.tar.gz` + `.sha256`
- `oci2git-darwin-aarch64.tar.gz` + `.sha256`
- `oci2git-windows-x86_64.exe.tar.gz` + `.sha256`
- `oci2git_VERSION_amd64.deb`
- `oci2git_VERSION_arm64.deb`
- `debian-checksums.txt`

## Support

For issues: https://github.com/virviil/oci2git/issues
