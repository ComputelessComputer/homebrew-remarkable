class Remarkable < Formula
  desc "Sync your reMarkable tablet notes to a local folder"
  homepage "https://github.com/ComputelessComputer/remarkable-cli"
  url "https://github.com/ComputelessComputer/remarkable-cli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "cbf6ee81097af81e45fe88082640ce3e7e64005c0e1cd054359fb3eb743388ee"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/remarkable --version")
  end
end
