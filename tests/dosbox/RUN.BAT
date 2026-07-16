@echo off
rem ======================================================================
rem RUN.BAT - runs the full Noctis IV Plus universe-generation test suite
rem          inside DOS (invoked by the macOS wrapper run-niv-tests.sh).
rem
rem Produces, for each test set, a deterministic hash (FNV-1a + CRC32) of
rem the generated data plus optional raw binary dumps. Output goes to files
rem in the working dir (D:). The FNV/CRC reference vectors and the Borland
rem RNG sequence are checked by `selftest` first; any mismatch aborts.
rem
rem Coordinate fixtures below are derived offline with `find` and are
rem stable: the generator is a pure function of (x,y,z). They cover every
rem star class (0..11) and every planet type (0..9).
rem ======================================================================

echo [run] selftest ...
NIVTEST.EXE selftest -o D:\T_SELF.OUT
if errorlevel 1 goto abort

rem ---- TEST SET 1 : SOLAR SYSTEMS ---------------------------------------
rem One representative system per star class 0..11.
echo [run] test set 1 : solar systems ...
NIVTEST.EXE system -x 3812345 -y -4287654 -z -468962 -o D:\T1C00.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -342288 -o D:\T1C01.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -278951 -o D:\T1C02.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -152277 -o D:\T1C03.OUT
NIVTEST.EXE system -x 3812345 -y -4224317 -z -595636 -o D:\T1C04.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -25603  -o D:\T1C05.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z  481093 -o D:\T1C06.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -912321 -o D:\T1C07.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -785647 -o D:\T1C08.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -658973 -o D:\T1C09.OUT
NIVTEST.EXE system -x 3812345 -y -4287654 -z -595636 -o D:\T1C10.OUT
NIVTEST.EXE system -x 3812345 -y -4224317 -z  544430 -o D:\T1C11.OUT

rem ---- TEST SET 2 : PLANET TEXTURES -------------------------------------
rem One body of each planet type 0..9. moonid flag toggles moon path.
echo [run] test set 2 : planet textures ...
rem type 0 (rocky)  - class 9 system, body 20
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 20 -o D:\T2P00.OUT
rem type 1 (tiny)   - class 9 system, body 2
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 2  -o D:\T2P01.OUT
rem type 2 (desert) - class 4 system, body 22
NIVTEST.EXE planet -x 3812345 -y -4224317 -z -468962 -n 22 -o D:\T2P02.OUT
rem type 3 (ocean)  - class 9 system, body 23
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 23 -o D:\T2P03.OUT
rem type 4 (ice)    - class 9 system, body 25
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 25 -o D:\T2P04.OUT
rem type 5 (moon)   - class 9 system, body 19
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 19 -o D:\T2P05.OUT
rem type 6 (gas minor) - class 9 system, body 0
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 0  -o D:\T2P06.OUT
rem type 7 (gas giant) - class 9 system, body 26
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 26 -o D:\T2P07.OUT
rem type 8 (ringed) - class 9 system, body 31
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 31 -o D:\T2P08.OUT
rem type 9 (gas)    - class 9 system, body 5
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -n 5  -o D:\T2P09.OUT

rem ---- TEST SET 3 : SECTOR HEIGHTMAPS -----------------------------------
rem build_surface for a sample landing; covers a few classes.
echo [run] test set 3 : sector heightmaps ...
NIVTEST.EXE sector -x 3812345 -y -4287654 -z -658973 -n 7  -c 0 -m 0 -s 0 -o D:\T3A.OUT
NIVTEST.EXE sector -x 3812345 -y -4287654 -z -658973 -n 9  -c 1 -m 1 -s 1 -o D:\T3B.OUT
NIVTEST.EXE sector -x 3812345 -y -4224317 -z -468962 -n 5  -c 2 -m 0 -s 2 -o D:\T3C.OUT
NIVTEST.EXE sector -x 3812345 -y -4287654 -z -785647 -n 2  -c 3 -m 1 -s 3 -o D:\T3D.OUT

rem ---- TEST SET 4 : SURFACE TEXTURES ------------------------------------
rem surface() for planet and moon, a few types.
echo [run] test set 4 : surface textures ...
NIVTEST.EXE surftex -x 3812345 -y -4287654 -z -658973 -n 7  -o D:\T4A.OUT
NIVTEST.EXE surftex -x 3812345 -y -4287654 -z -658973 -n 26 -o D:\T4B.OUT
NIVTEST.EXE surftex -x 3812345 -y -4287654 -z -658973 -n 19 -o D:\T4C.OUT
NIVTEST.EXE surftex -x 3812345 -y -4224317 -z -468962 -n 5  -o D:\T4D.OUT

rem ---- TEST SET 5 : EXTRA (compact fingerprints + RNG) ------------------
echo [run] test set 5 : extra fingerprints ...
NIVTEST.EXE scan -x 3812345 -y -4287654 -z -658973 -o D:\T5A.OUT
NIVTEST.EXE scan -x 500 -y -600 -z 700 -o D:\T5B.OUT

echo [run] ALL TEST SETS DONE > D:\RUN.OK
goto done

:abort
echo [run] selftest FAILED - aborting > D:\RUN.FAIL

:done
