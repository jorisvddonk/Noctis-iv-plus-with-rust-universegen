@echo off
rem ======================================================================
rem BUILD.BAT - compiles the Noctis IV Plus generation test harness inside
rem             DOS (run under DOSBox by the macOS wrapper).
rem
rem Compiles the REAL, unmodified generation modules NOCTIS-0.CPP and
rem NOCTIS-1.CPP together with the harness, then links them into NIVTEST.EXE.
rem NOCTIS.CPP (the interactive game main) is deliberately excluded; the few
rem symbols it would provide are stubbed in NIVSTUBS.
rem
rem Drives (set up by the caller's dosbox conf):
rem   C: = Turbo/Borland C++ toolchain  (C:\BIN  C:\INCLUDE  C:\LIB)
rem   D: = working dir (this build dir; source + harness copied here)
rem
rem Flags mirror the game's NOCTIS.CFG as closely as Turbo C++ 3.0 allows:
rem   -ml  large memory model      (game uses -ml)
rem   -2   80286 instructions      (game uses -3; TC3.0 max is -2. The 32-bit
rem        parts of the generator are hand-assembled with 0x66 prefixes and
rem        are unaffected by the codegen target.)
rem   -f287 hardware FPU codegen   (game uses -f287; IEEE results identical)
rem   -O -Z optimizations          (game uses -O -Oe -Ob -Z)
rem ======================================================================

set CFLAGS=-ml -2 -f287 -O -Z -IC:\INCLUDE

echo [build] compiling generation module NOCTIS-0.CPP ...
c:\BIN\tcc.exe -c %CFLAGS% NOCTIS-0.CPP > D:\BUILD.LOG
if not exist NOCTIS-0.OBJ goto fail

echo [build] compiling generation module NOCTIS-1.CPP ...
c:\BIN\tcc.exe -c %CFLAGS% NOCTIS-1.CPP >> D:\BUILD.LOG
if not exist NOCTIS-1.OBJ goto fail

echo [build] compiling harness NIVTEST.CPP ...
c:\BIN\tcc.exe -c %CFLAGS% NIVTEST.CPP >> D:\BUILD.LOG
if not exist NIVTEST.OBJ goto fail

echo [build] compiling NIVHASH.C (as C++) ...
c:\BIN\tcc.exe -c -P %CFLAGS% NIVHASH.C >> D:\BUILD.LOG
if not exist NIVHASH.OBJ goto fail

echo [build] compiling NIVSTUBS.C (as C++) ...
c:\BIN\tcc.exe -c -P %CFLAGS% NIVSTUBS.C >> D:\BUILD.LOG
if not exist NIVSTUBS.OBJ goto fail

echo [build] linking NIVTEST.EXE ...
if exist NIVTEST.EXE del NIVTEST.EXE
c:\BIN\tlink.exe @LINK.RSP >> D:\BUILD.LOG
if not exist NIVTEST.EXE goto fail

echo [build] OK > D:\BUILD.OK
goto done

:fail
echo [build] FAILED > D:\BUILD.FAIL

:done
