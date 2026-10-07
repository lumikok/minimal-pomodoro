param(
    [Parameter(Position = 0)]
    [ValidateSet('setup', 'install', 'dev', 'check', 'test', 'build', 'verify-env')]
    [string]$Task = 'dev'
)

$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskDirectories = @('.tools/rustup', '.tools/cargo', '.tools/downloads', '.cache/npm', '.cache/tmp', '.cache/target', '.data/dev')
foreach ($taskDirectory in $taskDirectories) { New-Item -ItemType Directory -Force -Path (Join-Path $taskRoot $taskDirectory) | Out-Null }

$taskEnvironment = @{
    RUSTUP_HOME = (Join-Path $taskRoot '.tools/rustup')
    CARGO_HOME = (Join-Path $taskRoot '.tools/cargo')
    CARGO_TARGET_DIR = (Join-Path $taskRoot '.cache/target')
    npm_config_cache = (Join-Path $taskRoot '.cache/npm')
    npm_config_update_notifier = 'false'
    TEMP = (Join-Path $taskRoot '.cache/tmp')
    TMP = (Join-Path $taskRoot '.cache/tmp')
    PATH = ((Join-Path $taskRoot '.tools/cargo/bin') + [IO.Path]::PathSeparator + $env:PATH)
    POMODORO_DEV_DATA_DIR = (Join-Path $taskRoot '.data/dev')
}
$taskPrevious = @{}
foreach ($taskName in $taskEnvironment.Keys) {
    $taskPrevious[$taskName] = [Environment]::GetEnvironmentVariable($taskName, 'Process')
    [Environment]::SetEnvironmentVariable($taskName, $taskEnvironment[$taskName], 'Process')
}

function Invoke-ProjectCommand {
    param([string]$File, [string[]]$Arguments)
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File failed (exit $LASTEXITCODE)." }
}

Push-Location $taskRoot
try {
    switch ($Task) {
        'setup' {
            $taskRustup = Join-Path $taskRoot '.tools/downloads/rustup-init.exe'
            if (-not (Test-Path -LiteralPath $taskRustup)) {
                Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile $taskRustup
            }
            Invoke-ProjectCommand $taskRustup @('-y', '--no-modify-path', '--default-host', 'x86_64-pc-windows-msvc', '--default-toolchain', 'stable', '--profile', 'minimal')
        }
        'install' { Invoke-ProjectCommand 'npm.cmd' @('install', '--no-audit', '--no-fund') }
        'dev' { Invoke-ProjectCommand 'npm.cmd' @('run', 'tauri', '--', 'dev') }
        'check' { Invoke-ProjectCommand 'npm.cmd' @('run', 'build') }
        'test' { Invoke-ProjectCommand 'cargo' @('test', '--manifest-path', 'src-tauri/Cargo.toml') }
        'build' {
            Invoke-ProjectCommand 'npm.cmd' @('run', 'tauri', '--', 'build', '--bundles', 'nsis')
            $taskVersion = (Get-Content -LiteralPath (Join-Path $taskRoot 'package.json') -Raw | ConvertFrom-Json).version
            $taskInstallerName = "极简番茄钟_${taskVersion}_x64-setup.exe"
            $taskReleaseDirectory = Join-Path $taskRoot 'release'
            New-Item -ItemType Directory -Force -Path $taskReleaseDirectory | Out-Null
            Copy-Item -LiteralPath (Join-Path $taskRoot ".cache/target/release/bundle/nsis/$taskInstallerName") -Destination (Join-Path $taskReleaseDirectory $taskInstallerName)
            Write-Output "Installer copied to release/$taskInstallerName; build caches can be cleaned independently."
        }
        'verify-env' {
            foreach ($taskName in ($taskEnvironment.Keys | Sort-Object)) {
                if ($taskName -ne 'PATH') { Write-Output "$taskName=$([Environment]::GetEnvironmentVariable($taskName, 'Process'))" }
            }
            Get-Command node, git, cargo, rustc -ErrorAction SilentlyContinue | Select-Object Name, Source
        }
    }
}
finally {
    Pop-Location
    foreach ($taskName in $taskPrevious.Keys) {
        if ($null -eq $taskPrevious[$taskName]) {
            Remove-Item -LiteralPath "Env:\$taskName" -ErrorAction SilentlyContinue
        } else {
            [Environment]::SetEnvironmentVariable($taskName, $taskPrevious[$taskName], 'Process')
        }
    }
}
