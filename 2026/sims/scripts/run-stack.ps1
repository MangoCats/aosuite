# run-stack.ps1 — Launch the full Assign Onward component stack on localhost
#
# Starts all server components on fixed, non-conflicting ports:
#   ao-recorder  A  →  http://127.0.0.1:3000
#   ao-recorder  B  →  http://127.0.0.1:3010
#   ao-validator    →  http://127.0.0.1:4000
#   ao-exchange     →  http://127.0.0.1:3100 (if configured)
#   ao-relay        →  ws://127.0.0.1:3200
#   ao-pwa (dev)    →  http://127.0.0.1:5173
#
# Usage: .\scripts\run-stack.ps1 [-DataDir DIR] [-NoPwa]
param(
    [string]$DataDir = "$env:TEMP\ao-stack",
    [switch]$NoPwa,
    [switch]$Help
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SimsDir = Split-Path -Parent $ScriptDir
$Root2026 = Split-Path -Parent $SimsDir
$SrcDir = Join-Path $Root2026 "src"

# Port assignments
$PortRecorderA = 3000
$PortRecorderB = 3010
$PortValidator = 4000
$PortExchange  = 3100
$PortRelay     = 3200
$PortPwa       = 5173

if ($Help) {
    Write-Host "Usage: .\scripts\run-stack.ps1 [-DataDir DIR] [-NoPwa]" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Launches the full AO stack on localhost with these ports:"
    Write-Host "  Recorder A:  $PortRecorderA"
    Write-Host "  Recorder B:  $PortRecorderB"
    Write-Host "  Validator:   $PortValidator"
    Write-Host "  Exchange:    $PortExchange"
    Write-Host "  Relay:       $PortRelay"
    Write-Host "  PWA (dev):   $PortPwa"
    exit 0
}

# ── Find binaries ────────────────────────────────────────────────────
$BinDir = Join-Path $SrcDir "target\release"
if (-not (Test-Path (Join-Path $BinDir "ao-recorder.exe"))) {
    $BinDir = Join-Path $Root2026 "target\release"
}

function Find-Bin($name) {
    $path = Join-Path $BinDir "$name.exe"
    if (Test-Path $path) { return $path }
    return $null
}

$RecorderBin = Find-Bin "ao-recorder"
$ValidatorBin = Find-Bin "ao-validator"
$RelayBin = Find-Bin "ao-relay"

if (-not $RecorderBin) {
    Write-Host "ERROR: ao-recorder.exe not found. Run .\scripts\build.ps1 first." -ForegroundColor Red
    exit 1
}

Write-Host "=== Assign Onward — Full Stack Launcher ===" -ForegroundColor Cyan
Write-Host "Data directory: $DataDir"
Write-Host ""

# ── Create data directories ──────────────────────────────────────────
$dirs = @("recorder-a", "recorder-b", "validator", "exchange")
foreach ($d in $dirs) {
    $p = Join-Path $DataDir $d
    if (-not (Test-Path $p)) { New-Item -ItemType Directory -Path $p -Force | Out-Null }
}

# ── Generate random Ed25519 seeds ────────────────────────────────────
function New-HexSeed {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    return ($bytes | ForEach-Object { $_.ToString("x2") }) -join ''
}

$SeedA = if ($env:AO_SEED_A) { $env:AO_SEED_A } else { New-HexSeed }
$SeedB = if ($env:AO_SEED_B) { $env:AO_SEED_B } else { New-HexSeed }
$SeedV = if ($env:AO_SEED_V) { $env:AO_SEED_V } else { New-HexSeed }

# ── Write config files ───────────────────────────────────────────────
$recorderADir = Join-Path $DataDir "recorder-a"
$recorderBDir = Join-Path $DataDir "recorder-b"
$validatorDir = Join-Path $DataDir "validator"
$exchangeDir  = Join-Path $DataDir "exchange"

# Use forward slashes in TOML paths for cross-platform compat
$dataA = ($recorderADir -replace '\\','/') + "/data"
$dataB = ($recorderBDir -replace '\\','/') + "/data"
$dbV   = ($validatorDir -replace '\\','/') + "/validator.db"
$dbX   = ($exchangeDir  -replace '\\','/') + "/exchange_trades.db"

@"
host = "127.0.0.1"
port = $PortRecorderA
blockmaker_seed = "$SeedA"
data_dir = "$dataA"
dashboard = true

[[validators]]
url = "http://127.0.0.1:$PortValidator"
label = "local-validator"
"@ | Set-Content (Join-Path $recorderADir "recorder.toml") -Encoding UTF8

@"
host = "127.0.0.1"
port = $PortRecorderB
blockmaker_seed = "$SeedB"
data_dir = "$dataB"
dashboard = true

[[validators]]
url = "http://127.0.0.1:$PortValidator"
label = "local-validator"
"@ | Set-Content (Join-Path $recorderBDir "recorder.toml") -Encoding UTF8

@"
host = "127.0.0.1"
port = $PortValidator
db_path = "$dbV"
validator_seed = "$SeedV"
poll_interval_secs = 10
"@ | Set-Content (Join-Path $validatorDir "validator.toml") -Encoding UTF8

@"
db_path = "$dbX"
poll_interval_secs = 5
deposit_detection = "sse"
trade_ttl_secs = 300
"@ | Set-Content (Join-Path $exchangeDir "exchange.toml") -Encoding UTF8

# ── Start components as background jobs ──────────────────────────────
$jobs = @()

Write-Host "Starting components..." -ForegroundColor Yellow
Write-Host ""

# Recorder A
Write-Host "  [1/5] Recorder A -> http://127.0.0.1:$PortRecorderA"
$cfgA = Join-Path $recorderADir "recorder.toml"
$jobs += Start-Process -FilePath $RecorderBin -ArgumentList $cfgA -PassThru -NoNewWindow

# Recorder B
Write-Host "  [2/5] Recorder B -> http://127.0.0.1:$PortRecorderB"
$cfgB = Join-Path $recorderBDir "recorder.toml"
$jobs += Start-Process -FilePath $RecorderBin -ArgumentList $cfgB -PassThru -NoNewWindow

# Validator
if ($ValidatorBin) {
    Write-Host "  [3/5] Validator  -> http://127.0.0.1:$PortValidator"
    $cfgV = Join-Path $validatorDir "validator.toml"
    $jobs += Start-Process -FilePath $ValidatorBin -ArgumentList "run",$cfgV -PassThru -NoNewWindow
} else {
    Write-Host "  [3/5] Validator  -> SKIPPED (binary not found)" -ForegroundColor Yellow
}

# Relay
if ($RelayBin) {
    Write-Host "  [4/5] Relay      -> ws://127.0.0.1:$PortRelay"
    $jobs += Start-Process -FilePath $RelayBin -ArgumentList "--listen","127.0.0.1:$PortRelay" -PassThru -NoNewWindow
} else {
    Write-Host "  [4/5] Relay      -> SKIPPED (binary not found)" -ForegroundColor Yellow
}

# PWA dev server
if (-not $NoPwa -and (Test-Path (Join-Path $SrcDir "ao-pwa")) -and (Get-Command npm -ErrorAction SilentlyContinue)) {
    Write-Host "  [5/5] PWA (dev)  -> http://127.0.0.1:$PortPwa"
    $pwaDir = Join-Path $SrcDir "ao-pwa"
    $jobs += Start-Process -FilePath "npm" -ArgumentList "run","dev","--","--port",$PortPwa,"--host","127.0.0.1" -WorkingDirectory $pwaDir -PassThru -NoNewWindow
} else {
    Write-Host "  [5/5] PWA (dev)  -> SKIPPED (-NoPwa or npm not found)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host ("=" * 60) -ForegroundColor Cyan
Write-Host "  Assign Onward stack is running" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Recorder A:  http://127.0.0.1:$PortRecorderA"
Write-Host "  Recorder B:  http://127.0.0.1:$PortRecorderB"
Write-Host "  Validator:   http://127.0.0.1:$PortValidator"
Write-Host "  Relay:       ws://127.0.0.1:$PortRelay"
Write-Host "  PWA:         http://127.0.0.1:$PortPwa"
Write-Host ""
Write-Host "  Dashboard:   http://127.0.0.1:$PortRecorderA/dashboard"
Write-Host "  Health:      http://127.0.0.1:$PortRecorderA/health"
Write-Host ""
Write-Host "  Config dir:  $DataDir"
Write-Host ""
Write-Host "  Press Ctrl+C to stop all components" -ForegroundColor Yellow
Write-Host ("=" * 60) -ForegroundColor Cyan

# ── Wait and cleanup on Ctrl+C ──────────────────────────────────────
try {
    # Register cleanup
    $null = Register-EngineEvent -SourceIdentifier PowerShell.Exiting -Action {
        foreach ($j in $jobs) {
            if (-not $j.HasExited) { Stop-Process -Id $j.Id -Force -ErrorAction SilentlyContinue }
        }
    }

    # Wait for any process to exit
    while ($true) {
        $exited = $jobs | Where-Object { $_.HasExited }
        if ($exited) {
            Write-Host "A component exited unexpectedly. Shutting down..." -ForegroundColor Red
            break
        }
        Start-Sleep -Seconds 1
    }
} finally {
    Write-Host ""
    Write-Host "Shutting down all components..." -ForegroundColor Yellow
    foreach ($j in $jobs) {
        if (-not $j.HasExited) {
            Stop-Process -Id $j.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Host "All components stopped." -ForegroundColor Green
}
