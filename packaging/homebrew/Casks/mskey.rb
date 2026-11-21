cask "mskey" do
  version "0.2.7"
  sha256 "87fdc138fbbf3f1bc1ebc8e7f491c98ecca021b732f336ab432bf1da22c8b74a"

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
