# Builds the release program, its icons and menu, and the MSI.
#   ./installer/build.ps1
# Output: dist/FolderTint-<version>.msi (+ .sha256)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
Write-Host "FolderTint $version"

& "$root/scripts/cargo.ps1" build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$payload = Join-Path $root 'target/installer-payload'
Remove-Item -Recurse -Force $payload -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $payload | Out-Null
Copy-Item target/release/foldertint.exe $payload
Copy-Item LICENSE (Join-Path $payload 'LICENSE.txt')

# The icons and the context menu's WiX fragment come from the palette in
# the program itself. It is a GUI executable: wait for it explicitly.
$export = Start-Process -Wait -PassThru -NoNewWindow -FilePath (Join-Path $payload 'foldertint.exe') `
    -ArgumentList '--export-installer-assets', "`"$payload`""
if ($export.ExitCode -ne 0) { throw "exporting installer assets failed ($($export.ExitCode))" }
if (-not (Test-Path (Join-Path $payload 'ContextMenu.wxs'))) { throw 'ContextMenu.wxs was not generated' }

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$msi = Join-Path $dist "FolderTint-$version.msi"

# WiX 5.0.2 (MS-RL). 6 and 7 require the paid Open Source Maintenance Fee.
$wix = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wix) { throw 'WiX 5 not found: dotnet tool install --global wix --version 5.0.2' }
$wixVersion = (& wix --version) -replace '\+.*$', ''
if (-not $wixVersion.StartsWith('5.')) { throw "WiX $wixVersion found; this installer is built with WiX 5.0.2" }

& wix build installer/FolderTint.wxs (Join-Path $payload 'ContextMenu.wxs') -arch x64 `
    -d "Version=$version" -d "Payload=$payload" `
    -o $msi
if ($LASTEXITCODE -ne 0) { throw 'wix build failed' }

# A fixed name too, for the release's permanent "latest" link:
# releases/latest/download/FolderTint.msi
Copy-Item $msi (Join-Path $dist 'FolderTint.msi') -Force

$hash = (Get-FileHash $msi -Algorithm SHA256).Hash.ToLower()
"$hash  $(Split-Path $msi -Leaf)" | Set-Content "$msi.sha256" -Encoding ascii
Write-Host "Built $msi ($([math]::Round((Get-Item $msi).Length / 1KB)) KB)"
Write-Host "SHA-256 $hash"
