class MskeyCli < Formula
  desc "Command-line toolkit for MS Key Vietnamese text conversion and macros"
  homepage "https://github.com/buidangminh23/ms-key"
  version "0.1.2"
  license "MIT"

  head "https://github.com/buidangminh23/ms-key.git", branch: "main"

  on_macos do
    url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-macos-universal.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  on_linux do
    on_intel do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-amd64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end

    on_arm do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-arm64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
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
