<#
.SYNOPSIS
  Builds GlideDeck in release mode and packages it as a Windows MSI.

.DESCRIPTION
  Requires the WiX Toolset 3.x binaries (candle.exe, light.exe). Point -WixDir at
  them, set the WIX environment variable, or put them on PATH. The official
  binaries zip is available from https://github.com/wixtoolset/wix3/releases.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File packaging\windows\build-msi.ps1 -WixDir C:\tools\wix314
#>
param(
    [string]$WixDir = $env:WIX
)
# Native tools (cargo, WiX) report progress on stderr, so rely on exit codes.
$ErrorActionPreference = "Continue"
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path

function Find-Tool([string]$name) {
    foreach ($dir in @($WixDir, (Join-Path $WixDir "bin"))) {
        if ($dir -and (Test-Path (Join-Path $dir $name))) { return (Join-Path $dir $name) }
    }
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw "Could not find $name. Download the WiX 3 binaries and pass -WixDir <folder>."
}
$candle = Find-Tool "candle.exe"
$light = Find-Tool "light.exe"

# Version from the workspace Cargo.toml.
$version = (Select-String -Path "$repo\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value

Write-Host "Building GlideDeck $version (release)..."
Push-Location $repo
try {
    cargo build --release -p glidedeck
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally { Pop-Location }

$out = Join-Path $repo "dist"
$obj = Join-Path $repo "target\wix"
New-Item -ItemType Directory -Force $out, $obj | Out-Null
$exe = Join-Path $repo "target\release\glidedeck.exe"

& $candle -nologo -arch x64 -ext WixUIExtension `
    "-dVersion=$version" "-dExePath=$exe" "-dRepoDir=$repo" `
    -out "$obj\glidedeck.wixobj" "$repo\packaging\windows\glidedeck.wxs"
if ($LASTEXITCODE -ne 0) { throw "candle failed" }

$msi = Join-Path $out "GlideDeck-$version-x64.msi"
& $light -nologo -ext WixUIExtension -cultures:en-us -spdb `
    -out $msi "$obj\glidedeck.wixobj"
if ($LASTEXITCODE -ne 0) { throw "light failed" }

Write-Host "Created $msi"
