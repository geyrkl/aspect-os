# ASPECT OS - one-command live test drive.
# Default: QEMU runs headless inside WSL and serves VNC on 127.0.0.1:5901;
# scripts\vnc-viewer.ps1 (own RFB client, no dependencies) shows the desktop
# in a Windows window - mouse and keyboard work reliably there, because VNC
# pointer events are absolute and WSLg window capture is bypassed entirely.
# Close the viewer window (Alt+F4) to stop the OS.
#
# -Window    : old behaviour - QEMU window via WSLg (sdl). Mouse capture may
#              not work under WSLg, this is kept only for quick boot checks.
# -Fullscreen: start the viewer borderless maximized (F11 toggles anytime)
# -NoBuild   : skip the automatic ISO build
param(
    [switch]$Window,
    [switch]$Fullscreen,
    [switch]$NoBuild
)
$ErrorActionPreference = "Stop"
$repo = Split-Path $PSScriptRoot -Parent
$iso = Join-Path $repo "build\aspect-os.iso"

# --- build if the ISO is missing or older than the sources ---
if (-not $NoBuild) {
    $newest = Get-ChildItem "$repo\boot", "$repo\kernel", "$repo\Makefile" -Recurse -File |
              Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $stale = (-not (Test-Path $iso)) -or ($newest -and $newest.LastWriteTime -gt (Get-Item $iso).LastWriteTime)
    if ($stale) {
        Write-Host "[build] (re)building aspect-os.iso ..."
        $bs = (Get-Content -Raw "$PSScriptRoot\build.sh") -replace "`r", ""
        $bs | Set-Content -NoNewline -Encoding ASCII "\\wsl.localhost\Ubuntu-24.04\root\build-aspect.sh"
        wsl -d Ubuntu-24.04 -u root -- bash /root/build-aspect.sh
        if (-not (Test-Path $iso)) { throw "build failed - build/aspect-os.iso missing" }
    }
}

if ($Window) {
    $ss = (Get-Content -Raw "$PSScriptRoot\start-os.sh") -replace "`r", ""
    $ss | Set-Content -NoNewline -Encoding ASCII "\\wsl.localhost\Ubuntu-24.04\tmp\start-os.sh"
    wsl -d Ubuntu-24.04 -u root -- bash /tmp/start-os.sh
    return
}

# --- headless QEMU with VNC ---
$vs = (Get-Content -Raw "$PSScriptRoot\start-os-vnc.sh") -replace "`r", ""
$vs | Set-Content -NoNewline -Encoding ASCII "\\wsl.localhost\Ubuntu-24.04\tmp\start-os-vnc.sh"
wsl -d Ubuntu-24.04 -u root -- bash /tmp/start-os-vnc.sh

# --- wait for the VNC port (WSL2 forwards localhost to Windows) ---
$open = $false
for ($i = 0; $i -lt 100; $i++) {
    $t = New-Object Net.Sockets.TcpClient
    try { $t.Connect("127.0.0.1", 5901); $t.Close(); $open = $true; break }
    catch { $t.Close(); Start-Sleep -Milliseconds 300 }
}
if (-not $open) { throw "VNC port 5901 did not open - check WSL localhost forwarding" }

# --- own viewer window; closing it stops QEMU ---
Write-Host "[viewer] ASPECT OS window opens - F11 fullscreen, Alt+F4 quit"
if ($Fullscreen) {
    & powershell -ExecutionPolicy Bypass -File "$PSScriptRoot\vnc-viewer.ps1" -VncHost 127.0.0.1 -VncPort 5901 -Fullscreen
} else {
    & powershell -ExecutionPolicy Bypass -File "$PSScriptRoot\vnc-viewer.ps1" -VncHost 127.0.0.1 -VncPort 5901
}
wsl -d Ubuntu-24.04 -u root -- bash -c "kill `$(cat /tmp/aspect-os.pid) 2>/dev/null || true"
