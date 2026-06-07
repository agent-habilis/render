class AhR < Formula
  desc "render diagrams and documents as terminal text"
  homepage "https://github.com/agent-habilis/render"
  license "MIT"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/agent-habilis/render/releases/download/v#{version}/ah-r-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/agent-habilis/render/releases/download/v#{version}/ah-r-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    elsif Hardware::CPU.arm?
      url "https://github.com/agent-habilis/render/releases/download/v#{version}/ah-r-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    bin.install "ah-r"
  end

  test do
    assert_match "ah-r", shell_output("#{bin}/ah-r --version")
  end
end
