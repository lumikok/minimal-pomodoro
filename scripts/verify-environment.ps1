$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskVariables = @('PATH', 'RUSTUP_HOME', 'CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'npm_config_cache', 'POMODORO_DEV_DATA_DIR')
$taskBefore = @{}
foreach ($taskScope in @('Process', 'User', 'Machine')) {
    foreach ($taskName in $taskVariables) { $taskBefore["$taskScope/$taskName"] = [Environment]::GetEnvironmentVariable($taskName, $taskScope) }
}
& (Join-Path $PSScriptRoot 'project.ps1') verify-env
foreach ($taskScope in @('Process', 'User', 'Machine')) {
    foreach ($taskName in $taskVariables) {
        if ($taskBefore["$taskScope/$taskName"] -cne [Environment]::GetEnvironmentVariable($taskName, $taskScope)) {
            throw "Environment leaked: $taskScope/$taskName"
        }
    }
}
Write-Output 'PASS: process, user and machine environment values are unchanged after the project script.'
if (-not (Test-Path -LiteralPath (Join-Path $taskRoot '.tools/cargo/bin/rustc.exe'))) { throw 'Project-local Rust was not found.' }
if (-not (Test-Path -LiteralPath (Join-Path $taskRoot '.cache/npm/_logs'))) { throw 'Project-local npm cache was not found.' }
Write-Output 'PASS: Rust and npm caches are located in the project.'
