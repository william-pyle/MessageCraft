param(
    [ValidateSet('check', 'download')][string]$Action = 'check',
    [string]$Version = '',
    [string]$Url = '',
    [string]$Digest = ''
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

if ($Action -eq 'check') {
    $headers = @{ 'User-Agent' = 'MessageCraft-Updater'; 'Accept' = 'application/vnd.github+json' }
    $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/william-pyle/MessageCraft/releases/latest' -Headers $headers -TimeoutSec 20
    $asset = $release.assets | Where-Object { $_.name -eq 'MessageCraft-windows-x64.msi' } | Select-Object -First 1
    if (-not $asset -or -not $release.tag_name) { throw 'The latest release has no Windows installer.' }
    [pscustomobject]@{
        version = [string]$release.tag_name
        url = [string]$asset.browser_download_url
        digest = [string]$asset.digest
    } | ConvertTo-Json -Compress
    exit 0
}

if ($Url -notmatch '^https://github\.com/william-pyle/MessageCraft/releases/download/[^/]+/MessageCraft-windows-x64\.msi$') {
    throw 'Unexpected update download address.'
}
if ($Version -notmatch '^v?[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Unexpected release version.' }
$folder = Join-Path $env:LOCALAPPDATA 'MessageCraft\Updates'
New-Item -ItemType Directory -Force -Path $folder | Out-Null
$destination = Join-Path $folder ("MessageCraft-$Version.msi")
$temporary = "$destination.download"
try {
    Invoke-WebRequest -Uri $Url -OutFile $temporary -UseBasicParsing -TimeoutSec 120
    if ($Digest -match '^sha256:([0-9a-fA-F]{64})$') {
        $actual = (Get-FileHash -Path $temporary -Algorithm SHA256).Hash
        if ($actual -ine $Matches[1]) { throw 'The installer checksum did not match the release.' }
    }
    Move-Item -LiteralPath $temporary -Destination $destination -Force
    Write-Output $destination
} finally {
    Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
}
