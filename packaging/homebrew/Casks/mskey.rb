cask "mskey" do
  version "0.2.8"
  sha256 "1071187758601df2a26ab998a69878290fe6c78de1fa971d88a4090c54c68045"

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
