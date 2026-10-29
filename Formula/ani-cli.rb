class AniCli < Formula
  desc "Watch anime in your terminal - multi-provider TUI"
  homepage "https://github.com/Cherryo0/ani-cli"
  url "https://github.com/Cherryo0/ani-cli/archive/refs/tags/v0.1.7.tar.gz"
  sha256 "02fa0d3dafe7eb95e88d1b4f615a46812188a49c705df4a0633de7e7eaa11c63"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ani-cli", shell_output("#{bin}/ani-cli --help")
  end
end
