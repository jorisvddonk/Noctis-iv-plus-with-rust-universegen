#!/usr/bin/env bash
#
# fetch-toolchain.sh
#
# Downloads and assembles the Borland C++ 3.1 (DOS) toolchain used to
# compile the unmodified Noctis IV Plus generation code under DOSBox.
#
# The original toolchain is distributed as Borland "spanned" archives: a set
# of volumes named *.CA1, *.CA2, ... each carrying a 4-byte span header. A
# valid PKZIP archive is recovered by stripping the 4-byte prefix from every
# volume and concatenating them in numeric order. This script does exactly
# that, then extracts the compiler/linker/make/DPMI runtime into a layout
# the build expects:
#
#   <OUT>/BIN      bcc.exe tlink.exe tasm.exe make.exe cpp.exe + DPMI runtime
#   <OUT>/INCLUDE  standard + project headers
#   <OUT>/LIB      C0L.OBJ CL.LIB MATHL.LIB FP87.LIB EMU.LIB ...
#
# NOTE: Turbo C++ 3.0 (TCC) is no longer supported — use BCC only.
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

mkdir -p "$OUT_DIR/BCPP31/BIN" "$OUT_DIR/BCPP31/INCLUDE" "$OUT_DIR/BCPP31/LIB"
cd "$WORK"

echo "[fetch] downloading Borland C++ 3.1 spanned archive (bcpp31) ..."
URL="https://archive.org/download/bcpp31/bcpp31.zip"
if ! curl -fsSL "$URL" -o bcpp31.zip; then
	echo "[fetch] ERROR: download failed (network or item unavailable)." >&2
	echo "[fetch] Try placing bcpp31.zip manually in $OUT_DIR and re-run." >&2
	exit 1
fi

echo "[fetch] inspecting archive for spanned volumes ..."
VOLSETS=$(unzip -l bcpp31.zip | awk '{print $4}' | grep -iE '\.CA[0-9]+$' | sed -E 's/\.CA[0-9]+$//' | sort -u)

reconstruct() {
	local base="$1"
	local out="$2"
	local vols
	vols=$(unzip -l bcpp31.zip | awk '{print $4}' | grep -iE "^${base}\.CA[0-9]+\$" | sort -t. -k2 -n)
	: > "$out"
	for v in $vols; do
		unzip -p bcpp31.zip "$v" | tail -c +5 >> "$out"
	done
}

if [ -n "$VOLSETS" ]; then
	echo "[fetch] found Borland spanned volumes; reconstructing ..."
	for base in $VOLSETS; do
		echo "[fetch]   reconstruct $base"
		reconstruct "$base" "$base.zip"
		unzip -o -q "$base.zip" -d extracted 2>/dev/null || true
	done
else
	echo "[fetch] no spanned volumes detected; extracting directly ..."
	unzip -o -q bcpp31.zip -d extracted
fi

echo "[fetch] collecting compiler binaries + DPMI runtime ..."
find extracted -type f \( -iname 'bcc.exe' -o -iname 'tlink.exe' -o -iname 'tasm.exe' \
	-o -iname 'make.exe' -o -iname 'cpp.exe' -o -iname 'tlink32.exe' \
	-o -iname 'dpmi16bi.ovl' -o -iname 'dpmiinst.exe' -o -iname 'dpmi386.exe' \
	-o -iname 'dpmi*' \) | head -200 | xargs -I{} cp {} "$OUT_DIR/BCPP31/BIN/" 2>/dev/null || true

echo "[fetch] collecting headers ..."
find extracted -type f \( -iname '*.h' -o -iname '*.hpp' \) -exec cp -t "$OUT_DIR/BCPP31/INCLUDE" {} + 2>/dev/null || true

echo "[fetch] collecting libraries + startup objects ..."
find extracted -type f \( -iname '*.lib' -o -iname '*.obj' \) -exec cp -t "$OUT_DIR/BCPP31/LIB" {} + 2>/dev/null || true

echo "[fetch] done. Toolchain at: $OUT_DIR"
echo "[fetch] BIN: $(ls "$OUT_DIR/BCPP31/BIN" | tr '\n' ' ')"
echo "[fetch] LIB: $(ls "$OUT_DIR/BCPP31/LIB" | tr '\n' ' ')"
