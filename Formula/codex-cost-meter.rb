class CodexCostMeter < Formula
  desc "Measure Codex task token usage and estimated API-list-price cost"
  homepage "https://github.com/deinspanjer/codex-cost-meter"
  url "https://github.com/deinspanjer/codex-cost-meter/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "05f5c83e8bf640388094a31e835b7b7ae3c585c173a2320b334a2ef414f2a331"
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
