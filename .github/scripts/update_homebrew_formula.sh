#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
	echo "usage: $0 VERSION CHECKSUM_DIR FORMULA_PATH" >&2
	exit 64
fi

VERSION=${1#v}
CHECKSUM_DIR=$2
FORMULA_PATH=$3
PRODUCT_NAME=lsef
RELEASE_BASE_URL="https://github.com/Takayuki-Todo/lsef/releases/download/v#{VERSION}"

set -- "$CHECKSUM_DIR"/*.sha256
if [ ! -e "$1" ]; then
	echo "no checksum files found in $CHECKSUM_DIR" >&2
	exit 66
fi

checksum_for() {
	target=$1
	filename="$PRODUCT_NAME-$VERSION-$target.tar.gz"
	checksum=$(
		awk -v filename="$filename" '$2 == filename { print $1 }' "$CHECKSUM_DIR"/*.sha256
	)

	if [ -z "$checksum" ]; then
		echo "missing sha256 for $filename" >&2
		exit 66
	fi

	printf '%s\n' "$checksum"
}

DARWIN_AMD64_SHA=$(checksum_for x86_64-apple-darwin)
DARWIN_ARM64_SHA=$(checksum_for aarch64-apple-darwin)
LINUX_AMD64_SHA=$(checksum_for x86_64-unknown-linux-gnu)
LINUX_ARM64_SHA=$(checksum_for aarch64-unknown-linux-gnu)

mkdir -p "$(dirname "$FORMULA_PATH")"

cat > "$FORMULA_PATH" <<EOF
VERSION = "$VERSION"

class Lsef < Formula
  desc "Rust-based file listing tool inspired by ls"
  homepage "https://github.com/Takayuki-Todo/lsef"
  version VERSION
  license "MIT"

  if OS.mac? && Hardware::CPU.intel?
    url "$RELEASE_BASE_URL/lsef-#{VERSION}-x86_64-apple-darwin.tar.gz"
    sha256 "$DARWIN_AMD64_SHA"
  end

  if OS.mac? && Hardware::CPU.arm?
    url "$RELEASE_BASE_URL/lsef-#{VERSION}-aarch64-apple-darwin.tar.gz"
    sha256 "$DARWIN_ARM64_SHA"
  end

  if OS.linux? && Hardware::CPU.intel?
    url "$RELEASE_BASE_URL/lsef-#{VERSION}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "$LINUX_AMD64_SHA"
  end

  if OS.linux? && Hardware::CPU.arm?
    url "$RELEASE_BASE_URL/lsef-#{VERSION}-aarch64-unknown-linux-gnu.tar.gz"
    sha256 "$LINUX_ARM64_SHA"
  end

  def install
    bin.install "lsef"
    bash_completion.install "completions/bash/lsef"
    zsh_completion.install "completions/zsh/_lsef"
    fish_completion.install "completions/fish/lsef" => "lsef.fish"
  end

  test do
    assert_match "lsef #{version}", shell_output("#{bin}/lsef --version")
  end
end
EOF
