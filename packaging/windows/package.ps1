<#
.SYNOPSIS
  Build MessageCraft's Windows installer (MSI) from the programs in dist\.

.DESCRIPTION
  Run build.ps1 first (it calls this script itself when WiX is available). Produces
  dist\MessageCraft-windows-x64.msi: a per-machine installer into Program Files with a Start Menu
  shortcut, like MailCraft's.

  Needs WiX v5 with its UI and Util extensions. Either on PATH:
    dotnet tool install --global wix --version 5.0.2
    wix extension add -g WixToolset.UI.wixext/5.0.2
    wix extension add -g WixToolset.Util.wixext/5.0.2
  or in app\.tools\wix (dotnet tool install --tool-path .tools\wix wix --version 5.0.2).
#>
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$root = Resolve-Path (Join-Path $here '..\..')
$dist = Join-Path $root 'dist'

$wix = Get-Command wix -ErrorAction SilentlyContinue
if ($wix) { $wixExe = $wix.Source } else {
    $wixExe = Join-Path $root '.tools\wix\wix.exe'
    if (-not (Test-Path $wixExe)) { throw 'WiX is not installed (see the notes at the top of package.ps1).' }
    $dotnetDir = Join-Path $root '.tools\dotnet'
    if (Test-Path $dotnetDir) { $env:DOTNET_ROOT = $dotnetDir }
}
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'

# The version comes from Cargo.toml, so the app and the installer always agree.
$version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$msiVersion = ($version -split '[-+]')[0]

# The extensions the installer's dialogs need; wix keeps them in its own cache.
foreach ($ext in 'WixToolset.UI.wixext/5.0.2', 'WixToolset.Util.wixext/5.0.2') {
    $name = ($ext -split '/')[0]
    $have = & $wixExe extension list -g 2>$null | Select-String -SimpleMatch $name
    if (-not $have) { & $wixExe extension add -g $ext | Out-Null }
}

# The licence page: what MessageCraft is made of and the terms of each part, from NOTICE.md.
$notice = Get-Content (Join-Path $root 'NOTICE.md') -Raw
$escaped = $notice -replace '\\', '\\' -replace '\{', '\{' -replace '\}', '\}'
$escaped = ($escaped -replace '(?m)^#+\s*', '' -replace '\*\*', '' -replace '`', '')
$sb = New-Object System.Text.StringBuilder
foreach ($ch in $escaped.ToCharArray()) {
    $code = [int]$ch
    if ($ch -eq "`n") { [void]$sb.Append('\par ') }
    elseif ($ch -eq "`r") { }
    elseif ($code -gt 127) { [void]$sb.Append("\u$code?") }
    else { [void]$sb.Append($ch) }
}
$rtfPath = Join-Path $env:TEMP 'messagecraft-license.rtf'
"{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}\fs18 $($sb.ToString())}" | Set-Content -Path $rtfPath -Encoding ascii

$msi = Join-Path $dist 'MessageCraft-windows-x64.msi'
if (Test-Path $msi) { Remove-Item $msi -Force }
& $wixExe build (Join-Path $here 'messagecraft.wxs') -arch x64 `
    -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
    -d "Version=$msiVersion" -d "BinDir=$dist" `
    -d "IconPath=$(Join-Path $root 'assets\messagecraft.ico')" -d "LicenseRtf=$rtfPath" `
    -o $msi
if ($LASTEXITCODE -ne 0) { throw 'The installer failed to build.' }
Remove-Item (Join-Path $dist 'MessageCraft-windows-x64.wixpdb') -ErrorAction SilentlyContinue
Write-Host "Installer: $msi"
