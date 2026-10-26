class CodexCostMeter < Formula
  desc "Measure Codex task token usage and estimated API-list-price cost"
  homepage "https://github.com/deinspanjer/codex-cost-meter"
  url "https://github.com/deinspanjer/codex-cost-meter/archive/refs/tags/v1.4.1.tar.gz"
  sha256 "21b022d0b68d0354afb0958cbb780668363ed7a2589d878b40e07e80312a2848"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
    # Keep existing source pins installable until a completion-capable release.
    if version >= "1.3.0"
      generate_completions_from_executable(bin/"codex-cost-meter", "completions", shells: [:bash, :zsh])
    end
  end

  def caveats
    <<~EOS
      On macOS, to enable automatic title updates:
        codex-cost-meter schedule install

      To uninstall, first run `codex-cost-meter schedule remove`, then use
      `brew uninstall codex-cost-meter`; do not use the utility's self-uninstall.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/codex-cost-meter --version")
    if version >= "1.3.0"
      assert_path_exists bash_completion/"codex-cost-meter"
      assert_path_exists zsh_completion/"_codex-cost-meter"
    end
  end
end
