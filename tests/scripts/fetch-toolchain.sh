#!/usr/bin/env bash
#
# fetch-toolchain.sh
#
# Downloads and assembles the Borland/Turbo C++ 3.0 (DOS) toolchain used to
# compile the unmodified Noctis IV Plus generation code under DOSBox.
#
# The original toolchain is distributed as Borland "spanned" archives: a set
# of volumes named *.CA1, *.CA2, ... each carrying a 4-byte span header. A
# valid PKZIP archive is recovered by stripping the 4-byte prefix from every
# volume and concatenating them in numeric order. This script does exactly
# that, then extracts the compiler/linker/make/DPMI runtime into a layout
# the build expects:
#
#   <OUT>/BIN      tcc.exe tlink.exe tasm.exe make.exe cpp.exe + DPMI runtime
#   <OUT>/INCLUDE  standard + project headers
#   <OUT>/LIB      C0L.OBJ CL.LIB MATHL.LIB FP87.LIB EMU.LIB ...
#
# Usage:
#   fetch-toolchain.sh [OUT_DIR]
#
# Defaults OUT_DIR to ./niv-toolchain next to this script's parent (tests/).
#
set -euo pipefail

OUT_DIR="${1:-$(cd "$(dirname "$0")/.." && pwd)/niv-toolchain}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$OUT_DIR/TC/BIN" "$OUT_DIR/TC/INCLUDE" "$OUT_DIR/TC/LIB"
cd "$WORK"

echo "[fetch] downloading Turbo C++ 3.0 spanned archive (tc-3.0) ..."
# archive.org item; the .zip is itself a Borland-spanned set.
URL="https://archive.org/download/tc-3.0/tc-3.0.zip"
if ! curl -fsSL "$URL" -o tc-3.0.zip; then
	echo "[fetch] ERROR: download failed (network or item unavailable)." >&2
	echo "[fetch] You can place a manually restored TC3.0.zip in $OUT_DIR and re-run." >&2
	exit 1
fi

echo "[fetch] inspecting archive for spanned volumes ..."
# Borland spanned volumes are named like CMDLINE.CA1, CMDLINE.CA2, ...
VOLSETS=$(unzip -l tc-3.0.zip | awk '{print $4}' | grep -iE '\.CA[0-9]+$' | sed -E 's/\.CA[0-9]+$//' | sort -u)

reconstruct() {
	local base="$1"
	local out="$2"
	# gather volumes in numeric order
	local vols
	vols=$(unzip -l tc-3.0.zip | awk '{print $4}' | grep -iE "^${base}\.CA[0-9]+\$" | sort -t. -k2 -n)
	: > "$out"
	for v in $vols; do
		unzip -p tc-3.0.zip "$v" | tail -c +5 >> "$out"
	done
}

if [ -n "$VOLSETS" ]; then
	echo "[fetch] found Borland spanned volumes; reconstructing each PKZIP volume ..."
	for base in $VOLSETS; do
		echo "[fetch]   reconstruct $base"
		reconstruct "$base" "$base.zip"
		unzip -o -q "$base.zip" -d extracted 2>/dev/null || true
	done
else
	echo "[fetch] no spanned volumes detected; extracting directly ..."
	unzip -o -q tc-3.0.zip -d extracted
fi

echo "[fetch] collecting compiler binaries + DPMI runtime ..."
find extracted -type f \( -iname 'tcc.exe' -o -iname 'tlink.exe' -o -iname 'tasm.exe' \
	-o -iname 'make.exe' -o -iname 'cpp.exe' -o -iname 'tlink32.exe' \
	-o -iname 'bcc.exe' -o -iname 'dpmi16bi.ovl' -o -iname 'dpimiload.exe' \
	-o -iname 'dpmi*' \) -exec cp -t "$OUT_DIR/TC/BIN" {} + 2>/dev/null || true

echo "[fetch] collecting headers ..."
find extracted -type f \( -iname '*.h' -o -iname '*.hpp' \) -exec cp -t "$OUT_DIR/TC/INCLUDE" {} + 2>/dev/null || true

echo "[fetch] collecting libraries + startup objects ..."
find extracted -type f \( -iname '*.lib' -o -iname '*.obj' \) -exec cp -t "$OUT_DIR/TC/LIB" {} + 2>/dev/null || true

echo "[fetch] done. Toolchain at: $OUT_DIR"
echo "[fetch] BIN: $(ls "$OUT_DIR/TC/BIN" | tr '\n' ' ')"
echo "[fetch] LIB: $(ls "$OUT_DIR/TC/LIB" | tr '\n' ' ')"
