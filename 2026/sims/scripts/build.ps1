# build.ps1 — Build all Assign Onward 2026 components (Windows PowerShell)
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SimsDir = Split-Path -Parent $ScriptDir
$Root2026 = Split-Path -Parent $SimsDir
$SrcDir = Join-Path $Root2026 "src"

Write-Host "=== Assign Onward — Build All Components ===" -ForegroundColor Cyan
Write-Host "Root: $Root2026"
Write-Host ""

# ── 1. Rust toolchain check ────────────────────────────────────────────
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: cargo not found. Install Rust from https://rustup.rs" -ForegroundColor Red
    exit 1
}

$rustVer = & rustc --version
Write-Host "Rust: $rustVer"
Write-Host ""

# ── 2. Build all Rust binaries ─────────────────────────────────────────
Write-Host "── Building Rust workspace (release mode)..." -ForegroundColor Yellow
Push-Location $SrcDir
try {
    & cargo build --release --bin ao-recorder --bin ao-validator --bin ao-exchange --bin ao-relay --bin ao-cli
    if ($LASTEXITCODE -ne 0) { throw "Rust workspace build failed" }
} finally { Pop-Location }

Write-Host ""
Write-Host "── Building ao-sims..." -ForegroundColor Yellow
Push-Location $SimsDir
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "ao-sims build failed" }
} finally { Pop-Location }

Write-Host ""

# ── 3. Locate built binaries ───────────────────────────────────────────
$BinDir = Join-Path $SrcDir "target\release"
$SimsBinDir = Join-Path $SimsDir "target\release"

# Check alternate locations
if (-not (Test-Path (Join-Path $BinDir "ao-recorder.exe"))) {
    $AltBin = Join-Path $Root2026 "target\release"
    if (Test-Path (Join-Path $AltBin "ao-recorder.exe")) { $BinDir = $AltBin }
}

Write-Host "Binaries:" -ForegroundColor Green
foreach ($bin in @("ao-recorder", "ao-validator", "ao-exchange", "ao-relay", "ao-cli")) {
    foreach ($dir in @($BinDir, $SimsBinDir)) {
        $path = Join-Path $dir "$bin.exe"
        if (Test-Path $path) {
            Write-Host "  $bin  ->  $path"
            break
        }
    }
}
$simsExe = Join-Path $SimsBinDir "ao-sims.exe"
if (Test-Path $simsExe) { Write-Host "  ao-sims  ->  $simsExe" }

Write-Host ""

# ── 4. PWA dependencies ───────────────────────────────────────────────
$PwaDir = Join-Path $SrcDir "ao-pwa"
if (Test-Path $PwaDir) {
    Write-Host "── Installing ao-pwa dependencies..." -ForegroundColor Yellow
    if (Get-Command npm -ErrorAction SilentlyContinue) {
        Push-Location $PwaDir
        try {
            & npm install --silent 2>&1 | Out-Null
            Write-Host "  ao-pwa: npm install complete"
        } finally { Pop-Location }
    } else {
        Write-Host "  WARNING: npm not found. Skipping PWA setup." -ForegroundColor Yellow
        Write-Host "  Install Node.js from https://nodejs.org"
    }
}

# ── 5. Viewer PWA dependencies ────────────────────────────────────────
$ViewerDir = Join-Path $SimsDir "viewer"
$ViewerPkg = Join-Path $ViewerDir "package.json"
if (Test-Path $ViewerPkg) {
    Write-Host "── Installing viewer PWA dependencies..." -ForegroundColor Yellow
    if (Get-Command npm -ErrorAction SilentlyContinue) {
        Push-Location $ViewerDir
        try {
            & npm install --silent 2>&1 | Out-Null
            Write-Host "  viewer: npm install complete"
        } finally { Pop-Location }
    }
}

Write-Host ""
Write-Host "=== Build complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:"
Write-Host "  Run a simulation:  .\scripts\run-sim.ps1 minimal"
Write-Host "  Run full stack:    .\scripts\run-stack.ps1"
Write-Host "  See the guide:     Get-Content GUIDE.md"
