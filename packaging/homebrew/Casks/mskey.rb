cask "mskey" do
  version "0.2.6"
  sha256 "89a5096f422c2a3a0dbb7d5bce3b466ddec7e0b7dffc6fef64d7bbc1f695de6e"

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
