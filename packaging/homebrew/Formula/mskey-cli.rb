class MskeyCli < Formula
  desc "Command-line toolkit for MS Key Vietnamese text conversion and macros"
  homepage "https://github.com/buidangminh23/ms-key"
  version "0.2.6"
  license "MIT"

  head "https://github.com/buidangminh23/ms-key.git", branch: "main"

  on_macos do
    url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-macos-universal.tar.gz"
    sha256 "db73ab401b503eeffd6593f39cdbcc33544a6b32f503e73a875f5f9bb754a53f"
  end

  on_linux do
    on_intel do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-amd64.tar.gz"
      sha256 "0153d840eb9e79ee16bdcbf8c19c25c38d963a28da1afed195be74daeb7ec018"
    end

    on_arm do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-arm64.tar.gz"
      sha256 "602775b3a33e4177d4e8f4912fecdcb95def2150c4f80e92cbd3f7c7b04809b0"
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
