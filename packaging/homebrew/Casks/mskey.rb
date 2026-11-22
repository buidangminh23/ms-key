cask "mskey" do
  version "0.2.11"
  sha256 "d37f14d89e751e76bb3cd35744234f2d020ebb108a847f5853468d57326af934"

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
