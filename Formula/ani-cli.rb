class AniCli < Formula
  desc "Watch anime in your terminal - multi-provider TUI"
  homepage "https://github.com/Cherryo0/ani-cli"
  url "https://github.com/Cherryo0/ani-cli/archive/refs/tags/v0.1.5.tar.gz"
  sha256 "f61482eb8c3f95b96fdf1382f423238417e775b2370b4394623bf5e7677f4a5e"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ani-cli", shell_output("#{bin}/ani-cli --help")
  end
end
