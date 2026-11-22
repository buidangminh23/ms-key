class MskeyCli < Formula
  desc "Command-line toolkit for MS Key Vietnamese text conversion and macros"
  homepage "https://github.com/buidangminh23/ms-key"
  version "0.2.9"
  license "MIT"

  head "https://github.com/buidangminh23/ms-key.git", branch: "main"

  on_macos do
    url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-macos-universal.tar.gz"
    sha256 "ed6224d085bcc8abfd9fecbe54297b5c3c65936fec9d99e47a9c0bb54927a959"
  end

  on_linux do
    on_intel do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-amd64.tar.gz"
      sha256 "74c6f6598be505b216c1460d930ec11600d89b54fce13f7881755a2df819ca74"
    end

    on_arm do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-arm64.tar.gz"
      sha256 "79beccfd49ea4a21b9ae71c7b0246b5b7048553105d2c94026d012fd9e2b34ab"
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
