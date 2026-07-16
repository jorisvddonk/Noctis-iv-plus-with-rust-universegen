#!/usr/bin/env bash
#
# run-niv-tests.sh
#
# End-to-end runner for the Noctis IV Plus universe-generation test suite.
#
# What it does:
#   1. Assembles a DOS build working dir from the REAL, unmodified generation
#      sources (NOCTIS-0.CPP / NOCTIS-1.CPP) plus the test harness.
#   2. Cross-compiles + links inside DOSBox-X (here: plain dosbox) running in
#      an Apple `container` (Linux arm64). The toolchain is Turbo/Borland C++ 3.0.
#   3. Runs RUN.BAT which emits, for each of the 5 test sets, stable FNV-1a +
#      CRC32 hashes of the generated universe data (plus optional raw dumps).
#   4. Copies the .OUT result files back to <repo>/tests/results/.
#
# Requires: `container` (Apple container CLI), `dosbox` inside the Debian image,
#           curl. The toolchain is fetched by tests/scripts/fetch-toolchain.sh.
#
# Usage:
#   run-niv-tests.sh [--toolchain DIR] [--dump] [--rebuild]
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC="$REPO/source"
HAR="$REPO/tests/harness"
DOSBOX="$REPO/tests/dosbox"
TOOLCHAIN="${TOOLCHAIN:-$REPO/niv-toolchain}"
RESULTS="$REPO/tests/results"
DUMP=0
REBUILD=0

while [ $# -gt 0 ]; do
	case "$1" in
		--toolchain) TOOLCHAIN="$2"; shift 2 ;;
		--dump)      DUMP=1; shift ;;
		--rebuild)   REBUILD=1; shift ;;
		*) echo "unknown arg: $1" >&2; exit 2 ;;
	esac
done

if [ ! -d "$TOOLCHAIN/TC/BIN" ] || [ ! -f "$TOOLCHAIN/TC/BIN/tcc.exe" ]; then
	echo "[run] toolchain missing at $TOOLCHAIN; fetching ..."
	"$SCRIPT_DIR/fetch-toolchain.sh" "$TOOLCHAIN"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "[run] assembling build dir in $WORK ..."
cp "$SRC"/NOCTIS-0.CPP "$SRC"/NOCTIS-1.CPP "$WORK/"
cp "$SRC"/*.h "$SRC"/*.H "$WORK/" 2>/dev/null || true
cp "$HAR"/NIVTEST.CPP "$HAR"/NIVHASH.C "$HAR"/NIVHASH.H "$HAR"/NIVSTUBS.C "$WORK/"
cp "$DOSBOX"/BUILD.BAT "$DOSBOX"/LINK.RSP "$DOSBOX"/RUN.BAT "$WORK/"

# dosbox config: mount toolchain as C:, build dir as D:, run build+run.
# Inside the container the bind-mounted host paths are fixed at /w/TC (the
# toolchain, read-only) and /w/work (the build dir, rw). dosbox mounts those.
cat > "$WORK/niv.conf" <<EOF
[cpu]
cputype=386
cycles=max
[dos]
xms=true
ems=true
[autoexec]
mount c /w/TC
mount d /w/work
path C:\\BIN
d:
call BUILD.BAT
if exist BUILD.FAIL goto end
call RUN.BAT
:end
exit
EOF

echo "[run] launching DOSBox inside Apple container ..."
# The container mounts the toolchain (read-only) and the build dir (rw).
container run --rm \
	-v "$TOOLCHAIN/TC":/w/TC:ro \
	-v "$WORK":/w/work \
	docker.io/library/debian:bookworm-slim bash -lc '
		export SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy
		apt-get update -qq >/dev/null 2>&1
		DEBIAN_FRONTEND=noninteractive apt-get install -y -qq dosbox >/dev/null 2>&1
		dosbox -conf /w/work/niv.conf -exit
	'

if [ ! -f "$WORK/RUN.OK" ]; then
	echo "[run] FAILED. Build/run log:" >&2
	[ -f "$WORK/BUILD.LOG" ] && tail -30 "$WORK/BUILD.LOG" >&2
	[ -f "$WORK/RUN.FAIL" ] && cat "$WORK/RUN.FAIL" >&2
	exit 1
fi

mkdir -p "$RESULTS"
echo "[run] copying results to $RESULTS ..."
cp "$WORK"/T*.OUT "$RESULTS/" 2>/dev/null || true
cp "$WORK"/BUILD.LOG "$RESULTS/" 2>/dev/null || true
if [ "$DUMP" -eq 1 ]; then
	cp "$WORK"/*.DMP "$RESULTS/" 2>/dev/null || true
fi

echo "[run] OK. $(ls "$RESULTS"/T*.OUT 2>/dev/null | wc -l | tr -d ' ') result files."
echo "[run] selftest summary:"
grep -iE "crc=|fnv=" "$RESULTS/T_SELF.OUT" 2>/dev/null | head -4 || true
