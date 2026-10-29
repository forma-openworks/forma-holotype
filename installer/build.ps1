# Собирает установщик Forma: Holotype — target\installer\Forma-Holotype-Setup-<версия>.exe.
#
#   powershell -ExecutionPolicy Bypass -File installer\build.ps1
#
# Шаги: release со статическим CRT (на чужом компьютере не нужен Visual C++ Redistributable),
# картинки мастера из логотипа (installer\art) и Setup.exe через Inno Setup 6.7+
# (winget install JRSoftware.InnoSetup). -NoBuild берёт уже собранный exe.

param([switch]$NoBuild)

# Без $ErrorActionPreference = 'Stop': cargo пишет ход сборки в stderr, и PowerShell 5.1
# принял бы это за ошибку. Коды выхода проверяются явно.
$root = Split-Path -Parent $PSScriptRoot
$triple = 'x86_64-pc-windows-msvc'
$out = Join-Path $root 'target\installer'

function Fail([string]$message) {
    Write-Host $message -ForegroundColor Red
    exit 1
}

# Версия — из [workspace.package] в Cargo.toml.
$match = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $match) { Fail 'Не найдена версия в Cargo.toml' }
$version = $match.Matches[0].Groups[1].Value

$iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { Fail 'Не найден Inno Setup 6 (ISCC.exe): winget install JRSoftware.InnoSetup' }

if (-not $NoBuild) {
    # Статический CRT — только для этой сборки: она идёт в target\x86_64-pc-windows-msvc,
    # обычный target\release не пересобирается.
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
    cargo build --release --target $triple -p holotype --manifest-path (Join-Path $root 'Cargo.toml')
    $code = $LASTEXITCODE
    Remove-Item Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS
    if ($code) { Fail "cargo build завершился с кодом $code" }
}
$bin = Join-Path $root "target\$triple\release"
if (-not (Test-Path (Join-Path $bin 'forma-holotype.exe'))) { Fail "Нет $bin\forma-holotype.exe: соберите без -NoBuild" }

# Иконка установщика — та же, что build.rs вшил в программу.
$ico = Get-ChildItem (Join-Path $bin 'build\holotype-*\out\forma-holotype.ico') -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $ico) { Fail 'Не найдена forma-holotype.ico из build.rs' }

$art = Join-Path $out 'art'
cargo run --release --quiet --manifest-path (Join-Path $PSScriptRoot 'art\Cargo.toml') --target-dir (Join-Path $out 'art-build') -- (Join-Path $root 'assets\logo\holotype.webp') $art
if ($LASTEXITCODE) { Fail "Картинки мастера: код $LASTEXITCODE" }

& $iscc /Q "/DAppVersion=$version" "/DBinDir=$bin" "/DIconFile=$($ico.FullName)" "/DArtDir=$art" "/DOutputDir=$out" (Join-Path $PSScriptRoot 'holotype.iss')
if ($LASTEXITCODE) { Fail "Inno Setup завершился с кодом $LASTEXITCODE" }

Write-Host "Готово: $(Join-Path $out "Forma-Holotype-Setup-$version.exe")" -ForegroundColor Green
