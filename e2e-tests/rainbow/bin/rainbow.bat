@echo off
rem Get the directory with this script
set dp0=%~dp0

if "%1"=="--long-lines" (
  type "%dp0%\rainbow-long.out"
) else if "%1"=="--short" (
  type "%dp0%\rainbow-short.out"
) else (
  type "%dp0%\rainbow.out"
)
