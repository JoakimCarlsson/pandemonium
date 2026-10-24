# Installs pandemonium from a GitHub release.
#
#   irm https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.ps1 | iex
#
# Environment:
#   PANDEMONIUM_CHANNEL       stable or preview               (default: stable; nightly is the old name of preview)
#   PANDEMONIUM_VERSION       release to install, e.g. 0.2.0   (default: the latest release of the channel)
#   PANDEMONIUM_COMPONENT     editor or server                 (default: editor)
#   PANDEMONIUM_INSTALL_DIR   where the binary goes            (default: %LOCALAPPDATA%\Programs\pandemonium)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'JoakimCarlsson/pandemonium'
$Target = 'x86_64-pc-windows-msvc'
$Channel = if ($env:PANDEMONIUM_CHANNEL) { $env:PANDEMONIUM_CHANNEL } else { 'stable' }
if ($Channel -eq 'nightly') { $Channel = 'preview' }
$Slug = switch ($Channel) {
    'stable' { 'pandemonium' }
    'preview' { 'pandemonium-preview' }
    default { throw 'PANDEMONIUM_CHANNEL must be stable or preview' }
}
$Label = if ($Channel -eq 'preview') { 'Pandemonium Preview' } else { 'Pandemonium' }
$Version = if ($env:PANDEMONIUM_VERSION) { $env:PANDEMONIUM_VERSION } else { 'latest' }
$Component = if ($env:PANDEMONIUM_COMPONENT) { $env:PANDEMONIUM_COMPONENT } else { 'editor' }
$InstallDir = if ($env:PANDEMONIUM_INSTALL_DIR) { $env:PANDEMONIUM_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\$Slug" }
$Binary = switch ($Component) {
    'editor' { $Slug }
    'server' { "$Slug-server" }
    default { throw 'PANDEMONIUM_COMPONENT must be editor or server' }
}

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
Returns the version the newest preview release is tagged with, without the leading v.
#>
function Get-LatestPreviewVersion {
    $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases?per_page=100" -UseBasicParsing
    $preview = $releases | Where-Object { $_.prerelease -and -not $_.draft } | Select-Object -First 1
    if (-not $preview) { throw "$Repo has no preview release yet" }
    $preview.tag_name
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
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) "$Label.lnk"
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
    $link.TargetPath = $Exe
    $link.Save()
}

<#
.SYNOPSIS
Moves what nightly kept in its data folder into the one preview already has: the projects, sessions and the rest replace preview's, and the settings preview has already are left as they are.
#>
function Merge-NightlyData([string]$From, [string]$To) {
    $settings = 'window-settings.json', 'editor-settings.json', 'machine-settings'
    foreach ($entry in Get-ChildItem -Force $From) {
        if ($entry.Name -eq 'server.json') { continue }
        $target = Join-Path $To $entry.Name
        if (($settings -contains $entry.Name) -and (Test-Path $target)) { continue }
        Remove-Item -Recurse -Force $target -ErrorAction SilentlyContinue
        Move-Item $entry.FullName $target
    }
    Remove-Item -Recurse -Force $From
}

<#
.SYNOPSIS
Moves a nightly install over to preview: stops its server, carries its data folder across, and removes what nightly installed.
#>
function Move-NightlyToPreview {
    $old = 'pandemonium-nightly'
    $oldDir = Join-Path $env:LOCALAPPDATA "Programs\$old"
    $oldData = Join-Path $env:APPDATA $old
    $newData = Join-Path $env:APPDATA $Slug
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Pandemonium Nightly.lnk'
    if (-not ((Test-Path $oldDir) -or (Test-Path $oldData) -or (Test-Path $shortcut))) { return }
    Write-Host 'moving your nightly install over to preview'
    $credential = Join-Path $oldData 'server.json'
    if (Test-Path $credential) {
        $held = Get-Content $credential -Raw | ConvertFrom-Json
        if ($held.pid) { Stop-Process -Id $held.pid -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 500 }
    }
    if (Test-Path $oldData) {
        if (Test-Path $newData) { Merge-NightlyData $oldData $newData; Write-Host "moved your projects and sessions from $oldData to $newData" }
        else { Move-Item $oldData $newData; Write-Host "moved $oldData to $newData" }
    }
    Remove-Item -Recurse -Force $oldDir -ErrorAction SilentlyContinue
    Remove-Item -Force $shortcut -ErrorAction SilentlyContinue
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    $kept = ($current -split ';') | Where-Object { $_ -and $_ -ne $oldDir }
    [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
}

if (-not [Environment]::Is64BitOperatingSystem) { throw 'pandemonium needs 64-bit Windows' }
if ($Version -eq 'latest') { $Version = if ($Channel -eq 'preview') { Get-LatestPreviewVersion } else { Get-LatestVersion } }
$Version = $Version.TrimStart('v')

$name = "$Binary-$Version-$Target"
$tag = "v$Version"
$base = "https://github.com/$Repo/releases/download/$tag"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    Write-Host "downloading $Binary $Version for $Target"
    Invoke-WebRequest -Uri "$base/$name.zip" -OutFile "$tmp\$name.zip" -UseBasicParsing
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile "$tmp\SHA256SUMS" -UseBasicParsing
    Assert-Checksum "$tmp\$name.zip" "$tmp\SHA256SUMS"
    if ($Channel -eq 'preview') { Move-NightlyToPreview }

    Expand-Archive -Path "$tmp\$name.zip" -DestinationPath $tmp
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force "$tmp\$name\*" $InstallDir

    $exe = Join-Path $InstallDir "$Binary.exe"
    Add-ToUserPath $InstallDir
    if ($Component -eq 'editor') { Add-StartMenuShortcut $exe }
    Write-Host "installed $Binary $Version to $InstallDir"
}
finally {
    Remove-Item -Recurse -Force $tmp
}
