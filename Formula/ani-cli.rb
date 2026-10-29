class AniCli < Formula
  desc "Watch anime in your terminal - multi-provider TUI"
  homepage "https://github.com/Cherryo0/ani-cli"
  url "https://github.com/Cherryo0/ani-cli/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "e11f58c02393ca4c610cf98f142f764fe4e1ef11f0ec2f048c1db3538a9decd5"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ani-cli", shell_output("#{bin}/ani-cli --help")
  end
end
