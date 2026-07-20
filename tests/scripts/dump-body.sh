#!/usr/bin/env bash
# dump-body.sh - build the C harness and run a SINGLE planet body,
# preserving surfmap.bin (the raw 64800-byte surface map) for byte-level
# comparison against the Rust port.
#
# Usage:
#   dump-body.sh -x X -y Y -z Z -n BODY [-o OUTDIR]
#
# Requires dosbox-x (brew install dosbox-x).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC="$REPO/source"
HAR="$REPO/tests/harness"
DOSBOX="$REPO/tests/dosbox"
TOOLCHAIN="${TOOLCHAIN:-$REPO/niv-toolchain/BCPP31}"

X=""; Y=""; Z=""; N=""; OUTDIR="$REPO/tests/results/body-dump"
while [ $# -gt 0 ]; do
	case "$1" in
		-x) X="$2"; shift 2 ;;
		-y) Y="$2"; shift 2 ;;
		-z) Z="$2"; shift 2 ;;
		-n) N="$2"; shift 2 ;;
		-o) OUTDIR="$2"; shift 2 ;;
		*) echo "unknown arg: $1" >&2; exit 2 ;;
	esac
done
[ -z "$X" ] || [ -z "$Y" ] || [ -z "$Z" ] || [ -z "$N" ] && {
	echo "usage: dump-body.sh -x X -y Y -z Z -n BODY [-o OUTDIR]" >&2; exit 2; }

if [ -f "$TOOLCHAIN/BIN/BCC.EXE" ]; then
	TC_MOUNT="$TOOLCHAIN"
elif [ -f "$TOOLCHAIN/TC/BIN/BCC.EXE" ]; then
	TC_MOUNT="$TOOLCHAIN/TC"
else
	echo "[dump] toolchain not found at $TOOLCHAIN" >&2; exit 1
fi

DOSBOX_X="${DOSBOX_X:-$(which dosbox-x 2>/dev/null || echo '')}"
[ -z "$DOSBOX_X" ] && { echo "[dump] dosbox-x not found" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "[dump] assembling build dir in $WORK ..."
cp "$SRC"/NOCTIS-0.CPP "$SRC"/NOCTIS-1.CPP "$WORK/"
cp "$SRC"/*.h "$SRC"/*.H "$WORK/" 2>/dev/null || true
cp "$HAR"/NIVTEST.CPP "$HAR"/NIVHASH.C "$HAR"/NIVHASH.H "$HAR"/NIVSTUBS.C "$WORK/"
cp "$DOSBOX"/BUILD.BAT "$DOSBOX"/LINK.RSP "$WORK/"

cat > "$WORK/RUN.BAT" <<REOF
@echo off
NIVTEST.EXE planet -x $X -y $Y -z $Z -p $N -dump D:\\ -o D:\\BODY.OUT
if exist surfmap.bin echo DUMP OK > D:\\RUN.OK
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

echo "[dump] launching DOSBox-X (body $N) ..."
export SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy
"$DOSBOX_X" -conf "$WORK/niv.conf" -exit

if [ ! -f "$WORK/RUN.OK" ]; then
	echo "[dump] FAILED. Build log:" >&2
	[ -f "$WORK/BUILD.LOG" ] && tail -30 "$WORK/BUILD.LOG" >&2
	exit 1
fi

mkdir -p "$OUTDIR"
cp "$WORK/surfmap.bin" "$OUTDIR/surfmap_${N}.bin"
cp "$WORK/BODY.OUT" "$OUTDIR/body_${N}.out"
echo "[dump] OK -> $OUTDIR/surfmap_${N}.bin ($(wc -c < "$OUTDIR/surfmap_${N}.bin" | tr -d ' ') bytes)"
