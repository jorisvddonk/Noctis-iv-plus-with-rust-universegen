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
rem Flags mirror the game's NOCTIS.CFG exactly:
rem   -ml   large memory model
rem   -3    80386 instructions
rem   -f287 hardware FPU codegen
rem   -O    optimize for speed (jump optimization)
rem   -Oe   register variables
rem   -Ob   inline built-in functions
rem   -Z    suppress register reloads
rem   -k-   no standard stack frame
rem   -h    fast huge pointers
rem   -vi-  no inline expansion of inline functions
rem   -wamp -wuse -weas -wpre  warning suppressions
rem ======================================================================

set CFLAGS=-ml -3 -f287 -O -Oe -Ob -Z -k- -h -vi- -wamp -wuse -weas -wpre -IC:\INCLUDE

rem compiler: BCC.EXE (Borland C++ 3.1) only
if exist C:\BIN\BCC.EXE set CC=C:\BIN\BCC.EXE
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
