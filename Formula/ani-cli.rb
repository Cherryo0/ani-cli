class AniCli < Formula
  desc "Watch anime in your terminal - multi-provider TUI"
  homepage "https://github.com/Cherryo0/ani-cli"
  url "https://github.com/Cherryo0/ani-cli/archive/refs/tags/v0.1.4.tar.gz"
  sha256 "7701921b92ebe352fa700f95be4c146aa3b40113080e1f293e82ee10e504eda1"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ani-cli", shell_output("#{bin}/ani-cli --help")
  end
end
