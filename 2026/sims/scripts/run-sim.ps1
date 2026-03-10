# run-sim.ps1 — Run an Assign Onward simulation by name (Windows PowerShell)
# Usage: .\scripts\run-sim.ps1 <scenario-name> [-ViewerPort PORT]
#
# Examples:
#   .\scripts\run-sim.ps1 minimal
#   .\scripts\run-sim.ps1 island-life -ViewerPort 4200
#   .\scripts\run-sim.ps1 all
param(
    [Parameter(Position=0)]
    [string]$Scenario,

    [int]$ViewerPort = 4200
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SimsDir = Split-Path -Parent $ScriptDir

$Simulations = @{
    "minimal"           = "Basic single-chain buy-redeem cycle (2 min)"
    "three-chain"       = "Multi-vendor trading on one recorder (3 min)"
    "exchange-3chain"   = "Cross-chain exchange mechanics (3 min)"
    "price-war"         = "Competitive exchange pricing dynamics (5 min)"
    "atomic-exchange"   = "CAA atomic cross-chain swaps (3 min)"
    "island-life"       = "Beach economy with map visualization (5 min)"
    "island-life-full"  = "Full island + validator + attacker (5 min)"
    "audit-adversarial" = "Five attack types vs validator (3 min)"
    "infra-resilience"  = "Server hardening verification (2 min)"
    "recorder-switch"   = "Recorder migration & owner key rotation (3 min)"
}

if (-not $Scenario -or $Scenario -eq "--help" -or $Scenario -eq "-h") {
    Write-Host "Usage: .\scripts\run-sim.ps1 <scenario-name> [-ViewerPort PORT]" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Available simulations:" -ForegroundColor Green
    foreach ($key in ($Simulations.Keys | Sort-Object)) {
        Write-Host ("  {0,-20} {1}" -f $key, $Simulations[$key])
    }
    Write-Host "  all                  Run every simulation sequentially"
    Write-Host ""
    Write-Host "While running, open http://127.0.0.1:$ViewerPort in your browser for the viewer."
    exit 0
}

# ── Find binary ───────────────────────────────────────────────────────
$SimsBin = $null
$candidates = @(
    (Join-Path $SimsDir "target\release\ao-sims.exe"),
    (Join-Path $SimsDir "target\debug\ao-sims.exe")
)
foreach ($c in $candidates) {
    if (Test-Path $c) { $SimsBin = $c; break }
}

if (-not $SimsBin) {
    Write-Host "ERROR: ao-sims.exe not found. Run .\scripts\build.ps1 first." -ForegroundColor Red
    exit 1
}

function Run-Scenario {
    param([string]$Name)

    $toml = Join-Path $SimsDir "scenarios\$Name.toml"
    if (-not (Test-Path $toml)) {
        Write-Host "ERROR: Scenario file not found: $toml" -ForegroundColor Red
        Write-Host "Run '.\scripts\run-sim.ps1 --help' to see available simulations."
        return
    }

    Write-Host ""
    Write-Host ("=" * 60) -ForegroundColor Cyan
    Write-Host "  Simulation: $Name" -ForegroundColor Cyan
    Write-Host "  Viewer UI:  http://127.0.0.1:5174" -ForegroundColor Cyan
    Write-Host "  Viewer API: http://127.0.0.1:$ViewerPort" -ForegroundColor Cyan
    Write-Host ("=" * 60) -ForegroundColor Cyan
    Write-Host ""

    # Start viewer PWA dev server in background
    $viewerDir = Join-Path $SimsDir "viewer"
    $viewerProc = $null
    if ((Test-Path (Join-Path $viewerDir "package.json")) -and (Get-Command npm -ErrorAction SilentlyContinue)) {
        $viewerProc = Start-Process -FilePath "npm" -ArgumentList "run","dev" -WorkingDirectory $viewerDir -PassThru -WindowStyle Hidden
    }

    Push-Location $SimsDir
    try {
        & $SimsBin $toml --viewer-port $ViewerPort
    } finally {
        Pop-Location
        if ($viewerProc -and -not $viewerProc.HasExited) {
            Stop-Process -Id $viewerProc.Id -Force -ErrorAction SilentlyContinue
        }
    }

    Write-Host ""
    Write-Host "── $Name complete ──" -ForegroundColor Green
}

if ($Scenario -eq "all") {
    Write-Host "=== Running all simulations sequentially ===" -ForegroundColor Cyan
    $tomlFiles = Get-ChildItem (Join-Path $SimsDir "scenarios\*.toml") | Sort-Object Name
    foreach ($f in $tomlFiles) {
        $name = $f.BaseName
        Run-Scenario $name
        Write-Host "Pausing 3 seconds before next simulation..."
        Start-Sleep -Seconds 3
    }
    Write-Host ""
    Write-Host "=== All simulations complete ===" -ForegroundColor Green
} else {
    Run-Scenario $Scenario
}
