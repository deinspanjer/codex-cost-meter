class CodexCostMeter < Formula
  desc "Measure Codex task token usage and estimated API-list-price cost"
  homepage "https://github.com/deinspanjer/codex-cost-meter"
  url "https://github.com/deinspanjer/codex-cost-meter/archive/refs/tags/v0.8.1.tar.gz"
  sha256 "5a9ea4e459eb6b6500d75b6cc29359184346bdd312cc4c2bd89d5387a05bef1e"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  def caveats
    <<~EOS
      To enable automatic title updates, and again after each Homebrew upgrade:
        codex-cost-meter schedule install

      To uninstall, first run `codex-cost-meter schedule remove`, then use
      `brew uninstall codex-cost-meter`; do not use the utility's self-uninstall.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/codex-cost-meter --version")
  end
end
