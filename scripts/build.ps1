# Builds Pixl.
#   ./scripts/build.ps1              ready-to-run copy in dist/
#   ./scripts/build.ps1 -Installer   also the Windows installer (target/release/bundle/nsis)
# Needs Rust and Node.js on PATH.

param([switch]$Installer)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

function Step([scriptblock]$run) {
    & $run
    if ($LASTEXITCODE -ne 0) { Write-Host 'Build failed.' -ForegroundColor Red; exit $LASTEXITCODE }
}

Push-Location settings
if (-not (Test-Path node_modules)) { Step { npm ci } }
if ($Installer) {
    # Builds the page, the tray app and the settings app, then packs the installer.
    # Update packages are signed when the updater key is available.
    $keyDir = Join-Path $HOME '.pixl'
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY -and (Test-Path "$keyDir/updater.key")) {
        $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$keyDir/updater.key" -Raw
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$keyDir/updater-key-password.txt" -Raw
    }
    if ($env:TAURI_SIGNING_PRIVATE_KEY) {
        Step { npm run tauri build }
    } else {
        Step { npm run tauri build -- --config '{\"bundle\":{\"createUpdaterArtifacts\":false}}' }
    }
} else {
    Step { npm run build }
}
Pop-Location

Step { cargo build --release -p pixl -p pixl-settings }

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item target/release/pixl-tray.exe (Join-Path $dist 'Pixl.exe') -Force
Copy-Item target/release/pixl-settings.exe $dist -Force
# Only present with the GNU toolchain; MSVC builds link WebView2 statically.
if (Test-Path target/release/WebView2Loader.dll) { Copy-Item target/release/WebView2Loader.dll $dist -Force }

Write-Host "Done: $dist\Pixl.exe"
if ($Installer) { Get-ChildItem target/release/bundle/nsis/*.exe | ForEach-Object { Write-Host "Installer: $($_.FullName)" } }
