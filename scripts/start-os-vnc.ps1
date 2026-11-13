# ASPECT OS - start live via VNC (fallback: sdl window may not deliver
# relative mouse motion under WSLg). QEMU runs headless in WSL, listening
# on 127.0.0.1:5901, and scripts\vnc-viewer.ps1 - our own RFB client -
# opens the desktop in a Windows window. See start-os.ps1 for the default
# one-command flow (build + boot + viewer).
param(
    [switch]$Fullscreen
)
Copy-Item "$PSScriptRoot\start-os-vnc.sh" "\\wsl.localhost\Ubuntu-24.04\tmp\start-os-vnc.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash /tmp/start-os-vnc.sh

# wait for the VNC port (WSL2 forwards localhost to Windows)
$open = $false
for ($i = 0; $i -lt 100; $i++) {
    $t = New-Object Net.Sockets.TcpClient
    try { $t.Connect("127.0.0.1", 5901); $t.Close(); $open = $true; break }
    catch { $t.Close(); Start-Sleep -Milliseconds 300 }
}
if (-not $open) { throw "VNC port 5901 did not open - check WSL localhost forwarding" }

if ($Fullscreen) {
    & powershell -ExecutionPolicy Bypass -File "$PSScriptRoot\vnc-viewer.ps1" -Fullscreen
} else {
    & powershell -ExecutionPolicy Bypass -File "$PSScriptRoot\vnc-viewer.ps1"
}
wsl -d Ubuntu-24.04 -u root -- bash -c "kill `$(cat /tmp/aspect-os.pid) 2>/dev/null || true"
