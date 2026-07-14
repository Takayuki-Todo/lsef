#!/bin/sh
set -eu

TAG=$1
PRODUCT_NAME=lsef
TARGET=${2:-${TARGET:-$(rustc -Vv | awk '/^host: / { print $2; exit }')}}
RELEASE=$PRODUCT_NAME-$TAG-$TARGET

case "$TARGET" in
	*windows-msvc|*windows-gnu)
		BIN_NAME=$PRODUCT_NAME.exe
		;;
	*)
		BIN_NAME=$PRODUCT_NAME
		;;
esac

cargo build --release --locked --target "$TARGET"

mkdir -p "dist/$RELEASE"
cp LICENSE README.md README.ja.md "dist/$RELEASE"
cp -R completions docs "dist/$RELEASE"
cp "target/$TARGET/release/$BIN_NAME" "dist/$RELEASE/"
tar cvfz "dist/$RELEASE.tar.gz" -C dist "$RELEASE"
