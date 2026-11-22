class MskeyCli < Formula
  desc "Command-line toolkit for MS Key Vietnamese text conversion and macros"
  homepage "https://github.com/buidangminh23/ms-key"
  version "0.2.10"
  license "MIT"

  head "https://github.com/buidangminh23/ms-key.git", branch: "main"

  on_macos do
    url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-macos-universal.tar.gz"
    sha256 "118aedce1b86fc8c621c019cab9d469eda354598eee73c27aa81f4ed142cf83d"
  end

  on_linux do
    on_intel do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-amd64.tar.gz"
      sha256 "b496a0bf3beb2e8a1ce11d7c3af07a07d8098b4cfc0c75b31158edd40751aded"
    end

    on_arm do
      url "https://github.com/buidangminh23/ms-key/releases/download/v#{version}/mskey-cli-#{version}-linux-arm64.tar.gz"
      sha256 "0f146e23fd81835d8d537ffd2df3c0d401fe97323feb4a2cf9dced9fee2f6486"
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
