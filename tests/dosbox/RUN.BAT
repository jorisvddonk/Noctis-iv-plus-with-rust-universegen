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
rem One body of each planet type 0..9 (coords/body from the T1C*.OUT systems).
echo [run] test set 2 : planet textures ...
rem type 0 (rocky, internally hot) - body 20 (moon), -595636 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -595636 -p 20 -o D:\T2P00.OUT
rem type 1 (small solid)          - body 0, -278951 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -278951 -p 0  -o D:\T2P01.OUT
rem type 2 (thick-atmosphere)     - body 0, -152277 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -152277 -p 0  -o D:\T2P02.OUT
rem type 3 (ocean)                - body 32 (moon), -658973 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -p 32 -o D:\T2P03.OUT
rem type 4 (ice)                  - body 0, -25603 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -25603  -p 0  -o D:\T2P04.OUT
rem type 5 (thin-atmosphere moon) - body 1, 481093 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z  481093 -p 1  -o D:\T2P05.OUT
rem type 6 (gas minor)            - body 0, -658973 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -658973 -p 0  -o D:\T2P06.OUT
rem type 7 (gas giant)            - body 22 (moon), -152277 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -152277 -p 22 -o D:\T2P07.OUT
rem type 8 (ringed)               - body 1, -912321 system
NIVTEST.EXE planet -x 3812345 -y -4287654 -z -912321 -p 1  -o D:\T2P08.OUT
rem type 9 (gas)                  - body 0, 544430 system
NIVTEST.EXE planet -x 3812345 -y -4224317 -z  544430 -p 0  -o D:\T2P09.OUT

rem ---- TEST SET 3 : SECTOR HEIGHTMAPS -----------------------------------
rem build_surface for a sample landing; covers a few classes.
echo [run] test set 3 : sector heightmaps ...
NIVTEST.EXE sector -x 3812345 -y -4287654 -z -658973 -p 7  -lon 0 -lat 60 -o D:\T3A.OUT
NIVTEST.EXE sector -x 3812345 -y -4287654 -z -658973 -p 9  -lon 0 -lat 60 -o D:\T3B.OUT
NIVTEST.EXE sector -x 3812345 -y -4224317 -z -468962 -p 5  -lon 0 -lat 60 -o D:\T3C.OUT
NIVTEST.EXE sector -x 3812345 -y -4287654 -z -785647 -p 1  -lon 0 -lat 60 -o D:\T3D.OUT

rem ---- TEST SET 4 : SURFACE TEXTURES ------------------------------------
rem surface() for planet and moon, a few types.
echo [run] test set 4 : surface textures ...
NIVTEST.EXE surftex -x 3812345 -y -4287654 -z -658973 -p 7  -lon 0 -lat 60 -o D:\T4A.OUT
NIVTEST.EXE surftex -x 3812345 -y -4287654 -z -658973 -p 26 -lon 0 -lat 60 -o D:\T4B.OUT
NIVTEST.EXE surftex -x 3812345 -y -4287654 -z -658973 -p 19 -lon 0 -lat 60 -o D:\T4C.OUT
NIVTEST.EXE surftex -x 3812345 -y -4224317 -z -468962 -p 5  -lon 0 -lat 60 -o D:\T4D.OUT

rem ---- TEST SET 5 : EXTRA (compact fingerprints + RNG) ------------------
echo [run] test set 5 : extra fingerprints ...
NIVTEST.EXE scan -x 3812345 -y -4287654 -z -658973 -o D:\T5A.OUT
NIVTEST.EXE scan -x 500 -y -600 -z 700 -o D:\T5B.OUT

echo [run] ALL TEST SETS DONE > D:\RUN.OK
goto done

:abort
echo [run] selftest FAILED - aborting > D:\RUN.FAIL

:done
