#Requires -Version 7.2
param(
    [string]$OutDir = "dist",
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version) { throw "Cannot read workspace version from Cargo.toml" }

if (-not $SkipBuild) {
    & cargo build --release -p mskey-win
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root "target" }
$exe = Join-Path $targetDir "release\mskey-win.exe"
if (-not (Test-Path $exe)) { throw "Missing $exe" }

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue)?.Source
if (-not $makensis) {
    foreach ($candidate in @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe")) {
        if (Test-Path $candidate) { $makensis = $candidate; break }
    }
}
if (-not $makensis) { throw "makensis not found; install NSIS" }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$outAbs = (Resolve-Path $OutDir).Path
$setup = Join-Path $outAbs "MSKey-Setup-$version-windows-x64.exe"
$zip = Join-Path $outAbs "MSKey-$version-windows-x64-portable.zip"

& $makensis /V2 /INPUTCHARSET UTF8 "/DVERSION=$version" "/DSRC_EXE=$exe" "/DOUTFILE=$setup" "/DLICENSE_FILE=$(Join-Path $root 'LICENSE')" "/DNOTICES_FILE=$(Join-Path $root 'THIRD-PARTY-NOTICES.md')" (Join-Path $root "scripts\windows-installer.nsi")
if ($LASTEXITCODE -ne 0) { throw "makensis failed" }

$stage = Join-Path ([System.IO.Path]::GetTempPath()) "mskey-portable-$version"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item $exe (Join-Path $stage "MSKey.exe")
Copy-Item (Join-Path $root "LICENSE") (Join-Path $stage "LICENSE.txt")
Copy-Item (Join-Path $root "THIRD-PARTY-NOTICES.md") (Join-Path $stage "THIRD-PARTY-NOTICES.txt")
Remove-Item -Force $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip
Remove-Item -Recurse -Force $stage

Write-Output $setup
Write-Output $zip
