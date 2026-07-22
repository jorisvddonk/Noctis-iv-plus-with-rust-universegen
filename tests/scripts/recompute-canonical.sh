#!/usr/bin/env bash
# recompute-canonical.sh - build the C harness and run `planet-all` for a
# single star system in ONE DOSBox-X boot, capturing the canonical FNV/CRC
# hashes for EVERY body. This is the ONLY correct source of canonical hashes:
# BCPP 3.1 compiled + run under DOSBox-X.
#
# Usage:
#   recompute-canonical.sh -x X -y Y -z Z [-o OUTFILE]
#
# Requires dosbox-x (brew install dosbox-x).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC="$REPO/source"
HAR="$REPO/tests/harness"
DOSBOX="$REPO/tests/dosbox"
TOOLCHAIN="${TOOLCHAIN:-$REPO/niv-toolchain/BCPP31}"

X=""; Y=""; Z=""; OUTFILE=""
while [ $# -gt 0 ]; do
	case "$1" in
		-x) X="$2"; shift 2 ;;
		-y) Y="$2"; shift 2 ;;
		-z) Z="$2"; shift 2 ;;
		-o) OUTFILE="$2"; shift 2 ;;
		*) echo "unknown arg: $1" >&2; exit 2 ;;
	esac
done
[ -z "$X" ] || [ -z "$Y" ] || [ -z "$Z" ] && {
	echo "usage: recompute-canonical.sh -x X -y Y -z Z [-o OUTFILE]" >&2; exit 2; }
[ -z "$OUTFILE" ] && OUTFILE="$REPO/tests/results/canonical/star_${X}_${Y}_${Z}.out"

if [ -f "$TOOLCHAIN/BIN/BCC.EXE" ]; then
	TC_MOUNT="$TOOLCHAIN"
else
	echo "[canon] toolchain not found at $TOOLCHAIN" >&2; exit 1
fi

DOSBOX_X="${DOSBOX_X:-$(which dosbox-x 2>/dev/null || echo '')}"
[ -z "$DOSBOX_X" ] && { echo "[canon] dosbox-x not found" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "[canon] assembling build dir in $WORK ..."
cp "$SRC"/NOCTIS-0.CPP "$SRC"/NOCTIS-1.CPP "$WORK/"
cp "$SRC"/*.h "$SRC"/*.H "$WORK/" 2>/dev/null || true
cp "$HAR"/NIVTEST.CPP "$HAR"/NIVHASH.C "$HAR"/NIVHASH.H "$HAR"/NIVSTUBS.C "$WORK/"
cp "$DOSBOX"/BUILD.BAT "$DOSBOX"/LINK.RSP "$WORK/"

cat > "$WORK/RUN.BAT" <<REOF
@echo off
NIVTEST.EXE planet-all -x $X -y $Y -z $Z -o D:\\STAR.OUT
if exist STAR.OUT echo RUN OK > D:\\RUN.OK
REOF

cat > "$WORK/niv.conf" <<PEOF
[cpu]
cputype=486
cycles=max
[dos]
xms=true
ems=true
[autoexec]
mount c ${TC_MOUNT}
mount d "$WORK"
path C:\\BIN
d:
call BUILD.BAT
if exist BUILD.FAIL goto end
call RUN.BAT
:end
exit
PEOF

echo "[canon] launching DOSBox-X (planet-all @ $X $Y $Z) ..."
export SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy
"$DOSBOX_X" -conf "$WORK/niv.conf" -exit

if [ ! -f "$WORK/RUN.OK" ]; then
	echo "[canon] FAILED. Build log:" >&2
	[ -f "$WORK/BUILD.LOG" ] && tail -30 "$WORK/BUILD.LOG" >&2
	exit 1
fi

mkdir -p "$(dirname "$OUTFILE")"
cp "$WORK/STAR.OUT" "$OUTFILE"
echo "[canon] OK -> $OUTFILE"
