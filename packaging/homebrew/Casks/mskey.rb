cask "mskey" do
  version "0.2.10"
  sha256 "3248b4aaf1a53024ee53b3e75f072e29c2d69033c04c848cd33da47b4337a47e"

  url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/MSKey-#{version}-macos-universal.dmg"
  name "MS Key"
  desc "Lightweight open-source Vietnamese input method"
  homepage "https://github.com/buidangminh23/ms-key"

  depends_on macos: ">= :ventura"

  app "MSKey.app"

  zap trash: [
    "~/.config/mskey",
    "~/Library/Preferences/org.mskey.mac.plist",
  ]

  caveats <<~EOS
    MSKey.app is ad-hoc signed and not notarized.
    If macOS blocks launch, allow it in System Settings > Privacy & Security.
    Grant Accessibility permission in System Settings > Privacy & Security > Accessibility.
  EOS
end
