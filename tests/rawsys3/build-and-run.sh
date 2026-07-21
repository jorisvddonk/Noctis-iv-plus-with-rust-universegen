#!/usr/bin/env bash
# Build and run RAWSYS3 — OG Noctis IV Plus system-gen module.
# Usage: build-and-run.sh X Y Z
set -euo pipefail
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$REPO/source"
DIR="$(cd "$(dirname "$0")" && pwd)"
TOOLCHAIN="${TOOLCHAIN:-$REPO/niv-toolchain/BCPP31}"
X="${1:-0}"; Y="${2:-0}"; Z="${3:-0}"

WORK=$(mktemp -d)
cp "$SRC"/NOCTIS-0.CPP "$SRC"/NOCTIS-1.CPP "$DIR"/RAWSYS3.CPP "$DIR"/NIVSTUBS.CPP "$DIR"/LINK.RSP "$WORK/"
for f in "$SRC"/*.h "$SRC"/*.H; do cp "$f" "$WORK/" 2>/dev/null; done

cat > "$WORK/BUILD.BAT" <<'REOF'
@echo off
c:\bin\bcc -c -ml -3 -f287 -O -Z -k- -h -vi- -IC:\INCLUDE NOCTIS-0.CPP >NUL
if errorlevel 1 goto fail
c:\bin\bcc -c -ml -3 -f287 -O -Z -k- -h -vi- -IC:\INCLUDE NOCTIS-1.CPP >NUL
if errorlevel 1 goto fail
c:\bin\bcc -c -ml -3 -f287 -O -Z -k- -h -vi- -IC:\INCLUDE RAWSYS3.CPP >NUL
if errorlevel 1 goto fail
c:\bin\bcc -c -ml -3 -f287 -O -Z -k- -h -vi- -IC:\INCLUDE NIVSTUBS.CPP >NUL
if errorlevel 1 goto fail
c:\bin\tlink /x/i/n/c/P-/A=16/LC:\LIB @LINK.RSP >NUL
if errorlevel 1 goto fail
echo OK
goto end
:fail
echo FAIL
:end
REOF

cat > "$WORK/niv.conf" <<EOF
[cpu] cputype=486 cycles=max
[dos] xms=true ems=true
[autoexec]
mount c $TOOLCHAIN
mount d $WORK
path C:\\BIN
d:
call BUILD.BAT
if exist RAWSYS3.EXE RAWSYS3.EXE $X $Y $Z >D:\\OUT.TXT
exit
EOF

export SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy
"$(which dosbox-x)" -conf "$WORK/niv.conf" -exit 2>&1 | grep -v 'LOG:' | grep -v '^$'
cat "$WORK/OUT.TXT" 2>/dev/null || echo "[rawsys3] BUILD FAILED"
rm -rf "$WORK"
