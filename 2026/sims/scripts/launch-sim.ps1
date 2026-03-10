# launch-sim.ps1 — Launch a simulation + viewer in one command
# Usage: powershell -ExecutionPolicy Bypass -File scripts\launch-sim.ps1 [scenario]
#
# Example: powershell -ExecutionPolicy Bypass -File scripts\launch-sim.ps1 minimal
param(
    [string]$Scenario = "minimal"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SimsDir = Split-Path -Parent $ScriptDir
$ViewerDir = Join-Path $SimsDir "viewer"

# Use different ports each launch to avoid TIME_WAIT conflicts.
# Viewer API: 4200-4209, Viewer UI: 5174-5183
$baseApi = 4200
$baseUi  = 5174

function Find-FreePort($base, $range) {
    for ($i = 0; $i -lt $range; $i++) {
        $port = $base + $i
        $conn = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
        if (-not $conn) { return $port }
    }
    return 0
}

$apiPort = Find-FreePort $baseApi 10
$uiPort  = Find-FreePort $baseUi 10

if ($apiPort -eq 0 -or $uiPort -eq 0) {
    Write-Host "ERROR: Could not find free ports. Wait a minute for TIME_WAIT to clear." -ForegroundColor Red
    exit 1
}

# Find ao-sims binary
$SimsBin = Join-Path $SimsDir "target\release\ao-sims.exe"
if (-not (Test-Path $SimsBin)) {
    $SimsBin = Join-Path $SimsDir "target\debug\ao-sims.exe"
}
if (-not (Test-Path $SimsBin)) {
    Write-Host "ERROR: ao-sims.exe not found. Build first with: cargo build --release" -ForegroundColor Red
    exit 1
}

# Find scenario
$Toml = Join-Path $SimsDir "scenarios\$Scenario.toml"
if (-not (Test-Path $Toml)) {
    Write-Host "ERROR: scenarios\$Scenario.toml not found" -ForegroundColor Red
    exit 1
}

# Write a temporary vite config that uses the chosen ports
$viteOverride = Join-Path $ViewerDir "vite.config.launch.ts"
@"
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({
  plugins: [react()],
  server: {
    port: $uiPort,
    host: '127.0.0.1',
    strictPort: true,
    proxy: { '/api': { target: 'http://127.0.0.1:$apiPort', ws: true } },
  },
});
"@ | Set-Content $viteOverride -Encoding UTF8

# Cleanup function
function Stop-All {
    param($SimProc, $ViewerProc)
    Write-Host "`nShutting down..." -ForegroundColor Yellow
    if ($SimProc -and -not $SimProc.HasExited) {
        Stop-Process -Id $SimProc.Id -Force -ErrorAction SilentlyContinue
    }
    if ($ViewerProc -and -not $ViewerProc.HasExited) {
        taskkill /PID $ViewerProc.Id /T /F 2>$null | Out-Null
    }
    # Clean up temp config
    if (Test-Path $viteOverride) { Remove-Item $viteOverride -Force -ErrorAction SilentlyContinue }
    Write-Host "Done." -ForegroundColor Green
}

# 1. Start the simulation
Write-Host "Starting simulation: $Scenario" -ForegroundColor Cyan
$simArgs = "`"$Toml`" --viewer-port $apiPort"
$simProc = Start-Process -FilePath $SimsBin -ArgumentList $simArgs `
    -WorkingDirectory $SimsDir -PassThru -NoNewWindow
Start-Sleep -Seconds 2

# 2. Start the viewer PWA dev server with the override config
Write-Host "Starting viewer UI..." -ForegroundColor Cyan
$viewerProc = Start-Process -FilePath "cmd.exe" `
    -ArgumentList "/c","npx","vite","--config","vite.config.launch.ts" `
    -WorkingDirectory $ViewerDir -PassThru -NoNewWindow
Start-Sleep -Seconds 3

Write-Host ""
Write-Host ("=" * 50) -ForegroundColor Green
Write-Host "  Sim '$Scenario' is running!" -ForegroundColor Green
Write-Host "  Open: http://127.0.0.1:$uiPort" -ForegroundColor Green
Write-Host "  Press Enter to stop (or close this window)" -ForegroundColor Yellow
Write-Host ("=" * 50) -ForegroundColor Green
Write-Host ""

# 3. Wait — poll for sim exit or user pressing Enter
try {
    while (-not $simProc.HasExited) {
        if ([Console]::KeyAvailable) {
            $key = [Console]::ReadKey($true)
            if ($key.Key -eq 'Enter') { break }
        }
        Start-Sleep -Milliseconds 500
    }
} finally {
    Stop-All $simProc $viewerProc
}
