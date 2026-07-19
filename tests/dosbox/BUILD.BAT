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
rem   C: = Borland C++ 3.1 toolchain (C:\BIN  C:\INCLUDE  C:\LIB)
rem   D: = working dir (this build dir; source + harness copied here)
rem
rem Flags mirror the game's NOCTIS.CFG:
rem   -ml  large memory model
rem   -2   80286 instructions
rem   -f287 hardware FPU codegen
rem   -O -Z optimizations
rem ======================================================================

set CFLAGS=-ml -2 -f287 -O -Z -IC:\INCLUDE

rem auto-detect compiler: BCC.EXE (Borland C++ 3.1) or TCC.EXE (Turbo C++ 3.0)
if exist C:\BIN\BCC.EXE set CC=C:\BIN\BCC.EXE
if exist C:\BIN\TCC.EXE set CC=C:\BIN\TCC.EXE
if "%CC%"=="" goto fail

echo [build] compiling generation module NOCTIS-0.CPP ...
%CC% -c %CFLAGS% NOCTIS-0.CPP > D:\BUILD.LOG
if not exist NOCTIS-0.OBJ goto fail

echo [build] compiling generation module NOCTIS-1.CPP ...
%CC% -c %CFLAGS% NOCTIS-1.CPP >> D:\BUILD.LOG
if not exist NOCTIS-1.OBJ goto fail

echo [build] compiling harness NIVTEST.CPP ...
%CC% -c %CFLAGS% NIVTEST.CPP >> D:\BUILD.LOG
if not exist NIVTEST.OBJ goto fail

echo [build] compiling NIVHASH.C (as C++) ...
%CC% -c -P %CFLAGS% NIVHASH.C >> D:\BUILD.LOG
if not exist NIVHASH.OBJ goto fail

echo [build] compiling NIVSTUBS.C (as C++) ...
%CC% -c -P %CFLAGS% NIVSTUBS.C >> D:\BUILD.LOG
if not exist NIVSTUBS.OBJ goto fail

echo [build] linking NIVTEST.EXE ...
if exist NIVTEST.EXE del NIVTEST.EXE
c:\BIN\TLINK.EXE @LINK.RSP >> D:\BUILD.LOG
if not exist NIVTEST.EXE goto fail

echo [build] OK > D:\BUILD.OK
goto done

:fail
echo [build] FAILED > D:\BUILD.FAIL

:done
