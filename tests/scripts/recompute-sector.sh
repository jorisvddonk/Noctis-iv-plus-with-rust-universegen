#!/usr/bin/env bash
# recompute-sector.sh - build the C harness and run `sector` for ONE body /
# ONE landing, capturing the canonical heightmap + objectchart FNV/CRC hashes.
# This is the ONLY correct source of canonical sector hashes: BCPP 3.1
# compiled + run under DOSBox-X.
#
# Usage:
#   recompute-sector.sh -x X -y Y -z Z -p P -lon LON -lat LAT -o OUTFILE
#
# Requires dosbox-x (brew install dosbox-x).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC="$REPO/source"
HAR="$REPO/tests/harness"
DOSBOX="$REPO/tests/dosbox"
TOOLCHAIN="${TOOLCHAIN:-$REPO/niv-toolchain/BCPP31}"

X=""; Y=""; Z=""; P=""; LON=""; LAT=""; OUTFILE=""
while [ $# -gt 0 ]; do
	case "$1" in
		-x) X="$2"; shift 2 ;;
		-y) Y="$2"; shift 2 ;;
		-z) Z="$2"; shift 2 ;;
		-p) P="$2"; shift 2 ;;
		-lon) LON="$2"; shift 2 ;;
		-lat) LAT="$2"; shift 2 ;;
		-o) OUTFILE="$2"; shift 2 ;;
		*) echo "unknown arg: $1" >&2; exit 2 ;;
	esac
done
[ -z "$X" ] || [ -z "$Y" ] || [ -z "$Z" ] || [ -z "$P" ] && {
	echo "usage: recompute-sector.sh -x X -y Y -z Z -p P [-lon LON] [-lat LAT] -o OUTFILE" >&2; exit 2; }
[ -z "$OUTFILE" ] && OUTFILE="$REPO/tests/results/sectors/star_${X}_${Y}_${Z}_b${P}_${LON}_${LAT}.out"
[ -z "$LON" ] && LON=0
[ -z "$LAT" ] && LAT=60

if [ -f "$TOOLCHAIN/BIN/BCC.EXE" ]; then
	TC_MOUNT="$TOOLCHAIN"
else
	echo "[sector] toolchain not found at $TOOLCHAIN" >&2; exit 1
fi

DOSBOX_X="${DOSBOX_X:-$(which dosbox-x 2>/dev/null || echo '')}"
[ -z "$DOSBOX_X" ] && { echo "[sector] dosbox-x not found" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "[sector] assembling build dir in $WORK ..."
cp "$SRC"/NOCTIS-0.CPP "$SRC"/NOCTIS-1.CPP "$WORK/"
cp "$SRC"/*.h "$SRC"/*.H "$WORK/" 2>/dev/null || true
cp "$HAR"/NIVTEST.CPP "$HAR"/NIVHASH.C "$HAR"/NIVHASH.H "$HAR"/NIVSTUBS.C "$WORK/"
cp "$DOSBOX"/BUILD.BAT "$DOSBOX"/LINK.RSP "$WORK/"

cat > "$WORK/RUN.BAT" <<REOF
@echo off
NIVTEST.EXE sector -x $X -y $Y -z $Z -p $P -lon $LON -lat $LAT -o D:\\SECT.OUT
if exist SECT.OUT echo RUN OK > D:\\RUN.OK
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

echo "[sector] launching DOSBox-X (sector @ $X $Y $Z p=$P lon=$LON lat=$LAT) ..."
export SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy
"$DOSBOX_X" -conf "$WORK/niv.conf" -exit

if [ ! -f "$WORK/RUN.OK" ]; then
	echo "[sector] FAILED. Build log:" >&2
	[ -f "$WORK/BUILD.LOG" ] && tail -30 "$WORK/BUILD.LOG" >&2
	exit 1
fi

mkdir -p "$(dirname "$OUTFILE")"
cp "$WORK/SECT.OUT" "$OUTFILE"
echo "[sector] OK -> $OUTFILE"
