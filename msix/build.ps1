# Builds the Microsoft Store package: dist/FolderTintCM-<version>.msix
#
#   ./msix/build.ps1            # the Store identity, ready to upload
#   ./msix/build.ps1 -Layout    # also leaves the package unpacked, to register
#                               # for testing (needs Windows' Developer Mode):
#   Add-AppxPackage -Register target/layout/<full name>/AppxManifest.xml
#
# The layout folder is named after the package's full name, as a Store
# install is (WindowsApps\<full name>): the program and its menu tell they
# run from the package by that name.
#
# The identity comes from Partner Center → FolderTintCM → Product
# management → Product identity (not secret). The Store signs the package
# itself: upload it unsigned.
#
# makeappx.exe is in Microsoft.Windows.SDK.BuildTools (nuget.org), unpacked
# under %LOCALAPPDATA%\Programs\windows-sdk-buildtools; SDK_BUILDTOOLS
# overrides that.
param(
    [string]$IdentityName = 'CarlosM.FolderTintCM',
    [string]$Publisher = 'CN=D34B73EC-B420-4B4F-9127-4A1F90BF5793',
    [string]$PublisherDisplayName = 'CarlosM.',
    # The publisher id in the package family name (Partner Center, PFN).
    [string]$PublisherId = '8ja15k5gbymem',
    [switch]$Layout
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$tools = if ($env:SDK_BUILDTOOLS) { $env:SDK_BUILDTOOLS } else { Join-Path $env:LOCALAPPDATA 'Programs\windows-sdk-buildtools' }
$makeappx = Get-ChildItem $tools -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.DirectoryName -like '*\x64' } | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found under $tools" }

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
# The Store wants four parts and the last one 0.
$packageVersion = "$version.0"
Write-Host "FolderTintCM $version → MSIX $packageVersion ($IdentityName)"

& "$root/scripts/cargo.ps1" build --release --workspace
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$stage = Join-Path $root 'target/msix'
if (Test-Path $stage) { Get-ChildItem $stage | Remove-Item -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item target/release/foldertintcm.exe $stage
Copy-Item target/release/foldertintcm_menu.dll $stage
Copy-Item THIRD_PARTY_LICENSES.md $stage
Copy-Item LICENSE (Join-Path $stage 'LICENSE.txt')

# The menu's icons and the logos, drawn by the program itself. It is a GUI
# executable: wait for it explicitly.
$p = Start-Process -Wait -PassThru -NoNewWindow -FilePath (Join-Path $stage 'foldertintcm.exe') `
    -ArgumentList '--export-package-assets', "`"$stage`""
if ($p.ExitCode -ne 0) { throw 'exporting the package assets failed' }

$manifest = Get-Content msix/AppxManifest.xml -Raw
$manifest = $manifest.Replace('{{IdentityName}}', $IdentityName).
    Replace('{{Publisher}}', [Security.SecurityElement]::Escape($Publisher)).
    Replace('{{PublisherDisplayName}}', [Security.SecurityElement]::Escape($PublisherDisplayName)).
    Replace('{{Version}}', $packageVersion)
[IO.File]::WriteAllText((Join-Path $stage 'AppxManifest.xml'), $manifest, (New-Object Text.UTF8Encoding $false))

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$msix = Join-Path $dist "FolderTintCM-$version.msix"
& $makeappx.FullName pack /d $stage /p $msix /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'makeappx pack failed' }
Write-Host "Built $msix ($([math]::Round((Get-Item $msix).Length / 1KB)) KB)"

if ($Layout) {
    $full = "$($IdentityName)_$($packageVersion)_x64__$PublisherId"
    $layoutRoot = Join-Path $root 'target/layout'
    if (Test-Path $layoutRoot) { Remove-Item -Recurse -Force $layoutRoot }
    $dest = Join-Path $layoutRoot $full
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item (Join-Path $stage '*') $dest -Recurse
    Write-Host "Layout to register for testing: $dest"
}
