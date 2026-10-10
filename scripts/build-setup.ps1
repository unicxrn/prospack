# Builds "Prospack Setup", the Windows installer app, around the NSIS installer that
# `npx tauri build` produced for the main app. Run from the repo root on Windows:
#   pwsh scripts/build-setup.ps1
$ErrorActionPreference = "Stop"

$nsis = Get-ChildItem "src-tauri/target/release/bundle/nsis/Prospack_*_x64-setup.exe" | Select-Object -First 1
if (-not $nsis) { throw "No NSIS installer found. Run 'npx tauri build' first." }
$version = ($nsis.Name -replace '^Prospack_', '' -replace '_x64-setup\.exe$', '')

# Installed size for the progress bar: the app plus the engine sidecar (plus a little for the uninstaller).
$app = (Get-Item "src-tauri/target/release/prospack.exe").Length
$engine = (Get-ChildItem "src-tauri/binaries/prospack-engine-*.exe" | Select-Object -First 1).Length
$expected = $app + $engine + 200KB

New-Item -ItemType Directory -Force -Path "installer/src-tauri/payload" | Out-Null
Copy-Item $nsis.FullName "installer/src-tauri/payload/prospack-setup.exe" -Force
Set-Content -Path "installer/src-tauri/payload/size.txt" -Value $expected -NoNewline

# Keep the setup app's version in step with the app's.
$conf = Get-Content "installer/src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json
$conf.version = $version
$conf | ConvertTo-Json -Depth 20 | Set-Content "installer/src-tauri/tauri.conf.json"
(Get-Content "installer/src-tauri/Cargo.toml") -replace '^version = ".*"', "version = `"$version`"" | Set-Content "installer/src-tauri/Cargo.toml"

Push-Location installer
npm ci
npx tauri build --no-bundle
Pop-Location

New-Item -ItemType Directory -Force -Path dist | Out-Null
$out = "dist/Prospack-Setup-$version.exe"
Copy-Item "installer/src-tauri/target/release/prospack-setup.exe" $out -Force
Write-Host "Built $out"
