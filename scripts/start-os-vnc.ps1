# ASPECT OS - start live via VNC (fallback: sdl window may not deliver
# relative mouse motion under WSLg). QEMU runs headless in WSL, listening
# on 127.0.0.1:5901 - connect any VNC viewer from Windows.
Copy-Item "$PSScriptRoot\start-os-vnc.sh" "\\wsl.localhost\Ubuntu-24.04\tmp\start-os-vnc.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash /tmp/start-os-vnc.sh
Write-Host ""
Write-Host "Now open a VNC viewer (e.g. RealVNC Viewer, TightVNC) and connect to  127.0.0.1:5901"
