@echo off
rem ASPECT OS - start the VM with the OS running, one double-click.
rem
rem What happens:
rem   1. the ISO is (re)built if any kernel/boot source is newer than it
rem   2. QEMU boots the ISO headless inside WSL and serves VNC on 127.0.0.1:5901
rem   3. an ASPECT OS window opens on Windows (own VNC viewer, no installs)
rem
rem Mouse and keyboard work in the window; closing it (Alt+F4) stops the OS.
rem Optional flags (forwarded to start-os.ps1):
rem   -Fullscreen   start the window borderless maximized (F11 toggles)
rem   -NoBuild      skip the automatic ISO build
rem   -Window       QEMU window via WSLg instead of the VNC viewer
rem                 (mouse capture may not work there)
title ASPECT OS
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\start-os.ps1" %*
if errorlevel 1 pause
