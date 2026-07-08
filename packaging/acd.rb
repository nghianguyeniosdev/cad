# Reference Homebrew formula for `acd`.
#
# This is a template. The Release workflow (.github/workflows/release.yml)
# generates a filled-in `acd.rb` (real version + per-arch sha256) as a release
# asset on each `v*` tag. To publish:
#
#   1. Create a public tap repo: nghianguyeniosdev/homebrew-tap
#   2. Copy the generated acd.rb from the release into Formula/acd.rb there.
#   3. Users install with:
#        brew tap nghianguyeniosdev/tap
#        brew install acd
#
# The placeholders below (VERSION, SHA256s) are filled per release by CI.
class Acd < Formula
  desc "Download and verify AWS CodeArtifact generic-package assets"
  homepage "https://github.com/nghianguyeniosdev/cad"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/nghianguyeniosdev/cad/releases/download/v0.1.0/acd-0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_SHA256"
    end
    on_intel do
      url "https://github.com/nghianguyeniosdev/cad/releases/download/v0.1.0/acd-0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  def install
    bin.install "acd"
  end

  test do
    assert_match "acd", shell_output("#{bin}/acd version")
  end
end
