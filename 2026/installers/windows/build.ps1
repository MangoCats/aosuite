# Build Windows MSI installer for Assign Onward suite.
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - WiX Toolset v4+ (dotnet tool install --global wix)
#   - Or WiX v3: light.exe and candle.exe on PATH
#
# Usage:
#   .\build.ps1                     # Release build, native target
#   .\build.ps1 -Profile debug      # Debug build
#   .\build.ps1 -Target x86_64-pc-windows-msvc  # Explicit target

param(
    [string]$Profile = "release",
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkspaceDir = Resolve-Path "$ScriptDir\..\..\src"
$OutDir = "$ScriptDir\out"

# Build binaries
Write-Host "==> Building workspace ($Profile)..." -ForegroundColor Cyan

$cargoArgs = @("build", "--$Profile")
if ($Target) {
    $cargoArgs += @("--target", $Target)
    $BinDir = "$WorkspaceDir\target\$Target\$Profile"
} else {
    $BinDir = "$WorkspaceDir\target\$Profile"
}

Push-Location $WorkspaceDir
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }
} finally {
    Pop-Location
}

# Verify binaries exist
$binaries = @("ao.exe", "ao-recorder.exe", "ao-validator.exe", "ao-exchange.exe")
foreach ($bin in $binaries) {
    if (-not (Test-Path "$BinDir\$bin")) {
        throw "Binary not found: $BinDir\$bin"
    }
}

Write-Host "==> All binaries built in $BinDir" -ForegroundColor Green

# Build MSI
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$wxsFile = "$ScriptDir\aosuite.wxs"
$msiFile = "$OutDir\AssignOnward-0.1.0-x64.msi"

# Try WiX v4 (dotnet tool) first, fall back to v3
$wixV4 = Get-Command "wix" -ErrorAction SilentlyContinue

if ($wixV4) {
    Write-Host "==> Building MSI with WiX v4..." -ForegroundColor Cyan
    & wix build $wxsFile -o $msiFile -d "BinDir=$BinDir"
    if ($LASTEXITCODE -ne 0) { throw "WiX build failed" }
} else {
    # WiX v3 fallback
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
    $wixobjFile = "$OutDir\aosuite.wixobj"

    & candle.exe $wxsFile -o $wixobjFile -dBinDir="$BinDir" -arch x64
    if ($LASTEXITCODE -ne 0) { throw "candle.exe failed" }

    & light.exe $wixobjFile -o $msiFile -ext WixUIExtension
    if ($LASTEXITCODE -ne 0) { throw "light.exe failed" }
}

Write-Host ""
Write-Host "==> MSI installer built: $msiFile" -ForegroundColor Green
Write-Host "    Install with: msiexec /i `"$msiFile`"" -ForegroundColor Gray
