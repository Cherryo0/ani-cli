class AniCli < Formula
  desc "Watch anime in your terminal - multi-provider TUI"
  homepage "https://github.com/Cherryo0/ani-cli"
  url "https://github.com/Cherryo0/ani-cli/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "8fc2016fae4a0e6edb680a43461a04048c616219dc924f20f8e2e2acda5a3f8d"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ani-cli", shell_output("#{bin}/ani-cli --help")
  end
end
