# Installs pandemonium from a GitHub release.
#
#   irm https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.ps1 | iex
#
# Environment:
#   PANDEMONIUM_VERSION       release to install, e.g. 0.2.0   (default: latest)
#   PANDEMONIUM_INSTALL_DIR   where the editor goes            (default: %LOCALAPPDATA%\Programs\pandemonium)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'JoakimCarlsson/pandemonium'
$Target = 'x86_64-pc-windows-msvc'
$Version = if ($env:PANDEMONIUM_VERSION) { $env:PANDEMONIUM_VERSION } else { 'latest' }
$InstallDir = if ($env:PANDEMONIUM_INSTALL_DIR) { $env:PANDEMONIUM_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\pandemonium' }

<#
.SYNOPSIS
Returns the version the latest release is tagged with, without the leading v.
#>
function Get-LatestVersion {
    $response = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -UseBasicParsing
    $url = $response.BaseResponse.ResponseUri
    if (-not $url) { $url = $response.BaseResponse.RequestMessage.RequestUri }
    $path = $url.AbsolutePath
    if ($path -notmatch '/tag/v?([^/]+)$') { throw "$Repo has no published release yet" }
    $Matches[1]
}

<#
.SYNOPSIS
Throws unless the archive matches its line in the release's SHA256SUMS.
#>
function Assert-Checksum([string]$Archive, [string]$Sums) {
    $name = Split-Path $Archive -Leaf
    $line = Get-Content $Sums | Where-Object { $_ -match "\s$([regex]::Escape($name))$" } | Select-Object -First 1
    if (-not $line) { throw "$name is not listed in SHA256SUMS" }
    $expected = ($line -split '\s+')[0]
    $actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash
    if ($actual -ne $expected) { throw "checksum mismatch for $name" }
}

<#
.SYNOPSIS
Puts the install directory on the user's PATH if it is not there already.
#>
function Add-ToUserPath([string]$Dir) {
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($current -split ';') -contains $Dir) { return }
    [Environment]::SetEnvironmentVariable('Path', "$Dir;$current", 'User')
    Write-Host "added $Dir to your PATH; open a new terminal to pick it up"
}

<#
.SYNOPSIS
Creates a Start menu shortcut to the editor.
#>
function Add-StartMenuShortcut([string]$Exe) {
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Pandemonium.lnk'
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
    $link.TargetPath = $Exe
    $link.Save()
}

if (-not [Environment]::Is64BitOperatingSystem) { throw 'pandemonium needs 64-bit Windows' }
if ($Version -eq 'latest') { $Version = Get-LatestVersion }
$Version = $Version.TrimStart('v')

$name = "pandemonium-$Version-$Target"
$base = "https://github.com/$Repo/releases/download/v$Version"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    Write-Host "downloading pandemonium $Version for $Target"
    Invoke-WebRequest -Uri "$base/$name.zip" -OutFile "$tmp\$name.zip" -UseBasicParsing
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile "$tmp\SHA256SUMS" -UseBasicParsing
    Assert-Checksum "$tmp\$name.zip" "$tmp\SHA256SUMS"

    Expand-Archive -Path "$tmp\$name.zip" -DestinationPath $tmp
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force "$tmp\$name\*" $InstallDir

    $exe = Join-Path $InstallDir 'pandemonium.exe'
    Add-ToUserPath $InstallDir
    Add-StartMenuShortcut $exe
    Write-Host "installed pandemonium $Version to $InstallDir"
}
finally {
    Remove-Item -Recurse -Force $tmp
}
