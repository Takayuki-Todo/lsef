#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
	echo "usage: $0 VERSION" >&2
	exit 64
fi

case "$1" in
	v*) RELEASE_TAG=$1 ;;
	*) RELEASE_TAG="v$1" ;;
esac

found=0
for asset in dist/*.tar.gz
do
	if [ ! -e "$asset" ]; then
		continue
	fi

	found=1
	echo "Uploading $asset to $RELEASE_TAG"
	gh release upload --clobber "$RELEASE_TAG" "$asset"
done

if [ "$found" -eq 0 ]; then
	echo "no distribution archives found in dist" >&2
	exit 66
fi
