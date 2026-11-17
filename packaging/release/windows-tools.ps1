# Ensure Code OSS native modules can use the installed VS 2022 Spectre libraries.
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$components = @(
    'Microsoft.VisualStudio.Component.VC.Runtimes.x86.x64.Spectre',
    'Microsoft.VisualStudio.Component.VC.ATL.Spectre'
)
$ready = & $vswhere -latest -version '[17.0,18.0)' -products '*' -requires $components -property installationPath
if ($ready) { exit 0 }
$installation = & $vswhere -latest -version '[17.0,18.0)' -products '*' -property installationPath
if (-not $installation) { throw 'Visual Studio 2022 was not found' }
$installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\setup.exe'
$arguments = @('modify', '--installPath', "`"$installation`"", '--quiet', '--norestart')
foreach ($component in $components) { $arguments += @('--add', $component) }
$process = Start-Process -FilePath $installer -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -notin @(0, 3010)) { throw "Visual Studio modification failed: $($process.ExitCode)" }
$ready = & $vswhere -latest -version '[17.0,18.0)' -products '*' -requires $components -property installationPath
if (-not $ready) { throw 'The required Spectre libraries were not installed' }
