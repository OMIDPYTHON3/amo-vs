# AMO VS Windows build script
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$args = @("build")
if ($Configuration -eq "Release") { $args += "--release"; $profileDir = "release" } else { $profileDir = "debug" }

Write-Host "==> Building AMO VS ($Configuration)"
& cargo @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$dist = Join-Path $PSScriptRoot "dist"
if (-not (Test-Path $dist)) { New-Item -ItemType Directory -Path $dist | Out-Null }
Copy-Item (Join-Path $PSScriptRoot "target\$profileDir\amo-vs.exe") (Join-Path $dist "amo-vs.exe") -Force
Write-Host "==> Built $dist\amo-vs.exe"
