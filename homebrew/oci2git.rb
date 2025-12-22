class Oci2git < Formula
  desc "A tool to convert OCI images to Git repositories"
  homepage "https://github.com/virviil/oci2git"
  version "0.2.4"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/virviil/oci2git/releases/download/v#{version}/oci2git-darwin-aarch64.tar.gz"
      sha256 "REPLACE_WITH_ARM64_MACOS_SHA256"
    else
      url "https://github.com/virviil/oci2git/releases/download/v#{version}/oci2git-darwin-x86_64.tar.gz"
      sha256 "REPLACE_WITH_X86_64_MACOS_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
      url "https://github.com/virviil/oci2git/releases/download/v#{version}/oci2git-linux-aarch64.tar.gz"
      sha256 "REPLACE_WITH_ARM64_LINUX_SHA256"
    else
      url "https://github.com/virviil/oci2git/releases/download/v#{version}/oci2git-linux-x86_64.tar.gz"
      sha256 "REPLACE_WITH_X86_64_LINUX_SHA256"
    end
  end

  def install
    if OS.mac?
      if Hardware::CPU.arm?
        bin.install "oci2git-darwin-aarch64" => "oci2git"
      else
        bin.install "oci2git-darwin-x86_64" => "oci2git"
      end
    else
      if Hardware::CPU.arm?
        bin.install "oci2git-linux-aarch64" => "oci2git"
      else
        bin.install "oci2git-linux-x86_64" => "oci2git"
      end
    end
  end

  test do
    system "#{bin}/oci2git", "--help"
  end
end
