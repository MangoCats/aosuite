# Build Windows MSI installer for Assign Onward Sims.
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - Node.js / npm (for viewer PWA build)
#   - WiX Toolset v4+ (dotnet tool install --global wix)
#
# Usage:
#   .\build-sims.ps1                  # Release build
#   .\build-sims.ps1 -Profile debug   # Debug build

param(
    [string]$Profile = "release",
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SimsDir = Resolve-Path "$ScriptDir\..\..\sims"
$OutDir = "$ScriptDir\out"

# --- Build binary ---
Write-Host "==> Building ao-sims ($Profile)..." -ForegroundColor Cyan

$cargoArgs = @("build", "--$Profile")
if ($Target) {
    $cargoArgs += @("--target", $Target)
    $BinDir = "$SimsDir\target\$Target\$Profile"
} else {
    $BinDir = "$SimsDir\target\$Profile"
}

Push-Location $SimsDir
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }
} finally {
    Pop-Location
}

if (-not (Test-Path "$BinDir\ao-sims.exe")) {
    throw "Binary not found: $BinDir\ao-sims.exe"
}

# --- Build viewer PWA ---
Write-Host "==> Building sims viewer PWA..." -ForegroundColor Cyan

Push-Location "$SimsDir\viewer"
try {
    if (-not (Test-Path "node_modules")) {
        & npm install
        if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
    }
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw "Viewer build failed" }
} finally {
    Pop-Location
}

# --- Build MSI ---
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$wxsFile = "$ScriptDir\aosims.wxs"
$msiFile = "$OutDir\AssignOnward-Sims-0.1.0-x64.msi"
$ScenariosDir = "$SimsDir\scenarios"

$wixV4 = Get-Command "wix" -ErrorAction SilentlyContinue

if ($wixV4) {
    Write-Host "==> Building MSI with WiX v4..." -ForegroundColor Cyan
    & wix build $wxsFile -o $msiFile `
        -d "BinDir=$BinDir" `
        -d "ScenariosDir=$ScenariosDir"
    if ($LASTEXITCODE -ne 0) { throw "WiX build failed" }
} else {
    $candle = Get-Command "candle.exe" -ErrorAction SilentlyContinue
    $light = Get-Command "light.exe" -ErrorAction SilentlyContinue

    if (-not $candle -or -not $light) {
        Write-Host @"
ERROR: WiX Toolset not found. Install one of:
  - WiX v4: dotnet tool install --global wix
  - WiX v3: https://wixtoolset.org/releases/
"@ -ForegroundColor Red
        exit 1
    }

    Write-Host "==> Building MSI with WiX v3..." -ForegroundColor Cyan
    $wixobjFile = "$OutDir\aosims.wixobj"

    & candle.exe $wxsFile -o $wixobjFile `
        -dBinDir="$BinDir" `
        -dScenariosDir="$ScenariosDir" `
        -arch x64
    if ($LASTEXITCODE -ne 0) { throw "candle.exe failed" }

    & light.exe $wixobjFile -o $msiFile -ext WixUIExtension
    if ($LASTEXITCODE -ne 0) { throw "light.exe failed" }
}

Write-Host ""
Write-Host "==> MSI installer built: $msiFile" -ForegroundColor Green
Write-Host "    Install with: msiexec /i `"$msiFile`"" -ForegroundColor Gray
