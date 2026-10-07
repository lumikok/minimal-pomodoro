# The only system-wide development dependency. Requires permission to install software.
$ErrorActionPreference = 'Stop'
$taskVsWhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path -LiteralPath $taskVsWhere) {
    $taskInstalled = & $taskVsWhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 Microsoft.VisualStudio.Component.Windows11SDK.26100 -property installationPath
    if ($taskInstalled) { Write-Output "C++ Build Tools already installed: $taskInstalled"; exit 0 }
}
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskDownloadDirectory = Join-Path $taskRoot '.tools/downloads'
New-Item -ItemType Directory -Force -Path $taskDownloadDirectory | Out-Null
$taskInstaller = Join-Path $taskDownloadDirectory 'vs_BuildTools.exe'
Invoke-WebRequest -Uri 'https://aka.ms/vs/17/release/vs_BuildTools.exe' -OutFile $taskInstaller
$taskSignature = Get-AuthenticodeSignature -LiteralPath $taskInstaller
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'Microsoft Corporation') {
    throw 'Microsoft installer signature verification failed.'
}
$taskProcess = Start-Process -FilePath $taskInstaller -ArgumentList @('--quiet', '--wait', '--norestart', '--nocache', '--add', 'Microsoft.VisualStudio.Workload.VCTools', '--includeRecommended') -WindowStyle Hidden -Wait -PassThru
if ($taskProcess.ExitCode -notin @(0, 3010)) { throw "Build Tools installation failed (exit $($taskProcess.ExitCode))." }
Write-Output "Build Tools installer completed with exit $($taskProcess.ExitCode)."
