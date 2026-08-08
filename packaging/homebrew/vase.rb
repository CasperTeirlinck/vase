# Homebrew formula for vase (installs the CLI / menu-bar agent binary).
# The release workflow substitutes VERSION_PLACEHOLDER and SHA256_PLACEHOLDER
# and writes this into the homebrew-vase tap as Formula/vase.rb.

class Vase < Formula
  desc "Cross-platform manual tiling window manager"
  homepage "https://github.com/CasperTeirlinck/vase"
  version "VERSION_PLACEHOLDER"
  url "https://github.com/CasperTeirlinck/vase/releases/download/v#{version}/vase-v#{version}-macos.tar.gz"
  sha256 "SHA256_PLACEHOLDER"

  def install
    bin.install "vase"
  end

  test do
    assert_predicate bin/"vase", :exist?
  end
end
