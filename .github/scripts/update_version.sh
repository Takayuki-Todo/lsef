#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
	echo "usage: $0 VERSION" >&2
	exit 64
fi

TO_VERSION=${1#v}
TOML_TMP=Cargo.toml.tmp
LOCK_TMP=Cargo.lock.tmp

awk -v version="$TO_VERSION" '
	BEGIN { in_package = 0; updated = 0 }
	/^\[package\]/ { in_package = 1 }
	in_package && /^version = / && updated == 0 {
		print "version = \"" version "\""
		updated = 1
		next
	}
	{ print }
	END {
		if (updated == 0) {
			exit 1
		}
	}
' Cargo.toml > "$TOML_TMP"
mv "$TOML_TMP" Cargo.toml

awk -v version="$TO_VERSION" '
	BEGIN { in_package = 0; in_lsef = 0; updated = 0 }
	/^\[\[package\]\]/ {
		in_package = 1
		in_lsef = 0
	}
	in_package && /^name = "lsef"/ { in_lsef = 1 }
	in_lsef && /^version = / && updated == 0 {
		print "version = \"" version "\""
		updated = 1
		in_lsef = 0
		next
	}
	{ print }
	END {
		if (updated == 0) {
			exit 1
		}
	}
' Cargo.lock > "$LOCK_TMP"
mv "$LOCK_TMP" Cargo.lock
