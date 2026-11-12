# ASPECT OS - start live (QEMU window via WSLg)
Copy-Item "$PSScriptRoot\start-os.sh" "\\wsl.localhost\Ubuntu-24.04\tmp\start-os.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash /tmp/start-os.sh
