# Builds MessageCraft into dist\: MessageCraft.exe (the window, Rust) and messagecraft-bridge.exe
# (the connection helper, Go). Go is used from .tools\go when it isn't installed system-wide.
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
Set-Location $root

$go = Get-Command go -ErrorAction SilentlyContinue
if ($go) { $goExe = $go.Source } else {
    $goExe = Join-Path $root '.tools\go\bin\go.exe'
    if (-not (Test-Path $goExe)) { throw "Go isn't installed. Install it from https://go.dev/dl/ or unzip it into app\.tools\go." }
    $env:GOROOT = Join-Path $root '.tools\go'
    $env:GOPATH = Join-Path $root '.tools\gopath'
    $env:GOCACHE = Join-Path $root '.tools\gocache'
}
$env:GOTOOLCHAIN = 'local'

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null

Write-Host 'Building the connection helper...'
Push-Location (Join-Path $root 'bridge')
& $goExe build -trimpath -ldflags '-s -w -H windowsgui' -o (Join-Path $dist 'messagecraft-bridge.exe') .
if ($LASTEXITCODE -ne 0) { Pop-Location; throw 'The connection helper failed to build.' }
Pop-Location

Write-Host 'Building the MessageCraft window...'
cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'MessageCraft failed to build.' }
Copy-Item (Join-Path $root 'target\release\MessageCraft.exe') $dist -Force
Copy-Item (Join-Path $root 'NOTICE.md') $dist -Force

Write-Host 'Packing the download and its page...'
$zip = Join-Path $dist 'MessageCraft-windows.zip'
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $dist 'MessageCraft.exe'), (Join-Path $dist 'messagecraft-bridge.exe'), (Join-Path $dist 'NOTICE.md') -DestinationPath $zip
Copy-Item (Join-Path $root 'web\*') $dist -Recurse -Force

# The installer (MSI), when WiX is available on PATH or in .tools\wix.
if ((Get-Command wix -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $root '.tools\wix\wix.exe'))) {
    Write-Host 'Building the installer...'
    & (Join-Path $root 'packaging\windows\package.ps1')
} else {
    Write-Warning 'WiX is not installed, so no installer was built (see packaging\windows\package.ps1).'
}
Write-Host "Done: $dist\MessageCraft.exe and $dist\index.html"
