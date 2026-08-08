# Homebrew cask for vase (installs vase.app to /Applications).
# The release workflow substitutes VERSION_PLACEHOLDER and SHA256_PLACEHOLDER
# and writes this into the homebrew-vase tap as Casks/vase.rb.

cask "vase" do
  version "VERSION_PLACEHOLDER"
  sha256 "SHA256_PLACEHOLDER"

  url "https://github.com/CasperTeirlinck/vase/releases/download/v#{version}/vase-v#{version}-macos.zip"
  name "vase"
  desc "Cross-platform manual tiling window manager"
  homepage "https://github.com/CasperTeirlinck/vase"

  app "vase.app"
end
