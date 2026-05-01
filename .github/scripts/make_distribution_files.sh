#! /bin/sh
TAG=$1
PRODUCT_NAME=lsef
RELEASE=$PRODUCT_NAME-$TAG-arm64-darwin
cargo build --release
mkdir -p dist/$RELEASE
cp LICENSE README.md target/release/lsef dist/$RELEASE
tar cvfz dist/$RELEASE.tar.gz -C dist $RELEASE