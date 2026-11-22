class MskeyCli < Formula
  desc "Command-line toolkit for MS Key Vietnamese text conversion and macros"
  homepage "https://github.com/buidangminh23/ms-key"
  version "0.2.11"
  license "MIT"

  head "https://github.com/buidangminh23/ms-key.git", branch: "main"

  on_macos do
    url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-macos-universal.tar.gz"
    sha256 "6f8d6db78a2e9790308908309c5a468e69404b7b93896f48facd0d5d7178bb94"
  end

  on_linux do
    on_intel do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-amd64.tar.gz"
      sha256 "7a9f2415524212bab720cd1b50aaea8a6b8666af23cb06140d9f001d466ff6d4"
    end

    on_arm do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-arm64.tar.gz"
      sha256 "4d48d1dd6d1d02b2d96540dd2c8950738ade5ea2454624e7603cfbee23c6de5b"
    end
  end

  head do
    depends_on "rust" => :build
  end

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args(path: "crates/mskey-cli")
    else
      bin.install "mskey"
    end
  end

  test do
    assert_match "mskey", shell_output("#{bin}/mskey --version")
    output = pipe_output("#{bin}/mskey convert --from unicode --to tcvn3", "tiếng việt\n")
    refute_empty output
  end
end
