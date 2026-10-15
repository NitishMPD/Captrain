$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$installation) { throw 'Install Visual Studio C++ Build Tools, including the x64 redistributable files.' }
$redist = Get-ChildItem (Join-Path $installation 'VC/Redist/MSVC') -Directory |
    Where-Object Name -Match '^\d+\.\d+\.\d+$' | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
$crt = Get-ChildItem (Join-Path $redist.FullName 'x64') -Directory -Filter 'Microsoft.VC*.CRT' | Select-Object -First 1 -ExpandProperty FullName
if (!(Test-Path (Join-Path $crt 'vcruntime140.dll'))) { throw "Missing x64 VC runtime at $crt" }
dotnet build (Join-Path $root 'recorder/Captrain.Recorder.csproj') -c Release -p:Platform=x64 -p:RestoreLockedMode=true
if ($LASTEXITCODE -ne 0) { throw 'Recorder build failed.' }
$output = Join-Path $root 'src-tauri/recorder'
Copy-Item -Path (Join-Path $crt '*.dll') -Destination $output
Copy-Item -LiteralPath (Join-Path $root 'recorder/ScreenRecorderLib.LICENSE.txt') -Destination $output
