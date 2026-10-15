$ErrorActionPreference = 'Stop'
# Analyze C# against the pinned Windows API assembly without emitting a binary.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installation = & $vswhere -latest -products '*' -property installationPath
if (!$installation) { throw 'Visual Studio Build Tools with Roslyn are required.' }
$roslyn = Join-Path $installation 'MSBuild/Current/Bin/Roslyn'
[AppDomain]::CurrentDomain.add_AssemblyResolve({ param($sender, $eventArgs)
    $path = Join-Path $roslyn (($eventArgs.Name.Split(',')[0]) + '.dll')
    if (Test-Path $path) { return [Reflection.Assembly]::LoadFrom($path) }
    return $null
})
foreach ($name in @('System.Runtime.CompilerServices.Unsafe', 'System.Memory', 'System.Buffers', 'System.Numerics.Vectors', 'System.Threading.Tasks.Extensions', 'System.Collections.Immutable', 'System.Reflection.Metadata', 'Microsoft.CodeAnalysis', 'Microsoft.CodeAnalysis.CSharp')) {
    [void][Reflection.Assembly]::LoadFrom((Join-Path $roslyn "$name.dll"))
}
$assets = Get-Content (Join-Path $PSScriptRoot 'obj/project.assets.json') -Raw | ConvertFrom-Json
$packages = $assets.packageFolders.PSObject.Properties.Name | Select-Object -First 1
$framework = Join-Path $packages 'microsoft.netframework.referenceassemblies.net48/1.0.3/build/.NETFramework/v4.8'
$paths = @('mscorlib', 'System', 'System.Core', 'System.Web.Extensions') | ForEach-Object { Join-Path $framework "$_.dll" }
$paths += Join-Path $packages 'screenrecorderlib/7.0.1/build/x64/ScreenRecorderLib.dll'
$refs = [Microsoft.CodeAnalysis.MetadataReference[]]@($paths | ForEach-Object { [Microsoft.CodeAnalysis.MetadataReference]::CreateFromFile($_) })
$text = Get-Content (Join-Path $PSScriptRoot 'Program.cs') -Raw
$tree = [Microsoft.CodeAnalysis.CSharp.CSharpSyntaxTree]::ParseText($text, $null, 'Program.cs', [Text.Encoding]::UTF8, [Threading.CancellationToken]::None)
$compilation = [Microsoft.CodeAnalysis.CSharp.CSharpCompilation]::Create('Captrain.Recorder.Check', [Microsoft.CodeAnalysis.SyntaxTree[]]@($tree), $refs, $null)
$compilation = $compilation.WithOptions($compilation.Options.WithPlatform([Microsoft.CodeAnalysis.Platform]::X64))
$errors = @($compilation.GetDiagnostics([Threading.CancellationToken]::None) | Where-Object Severity -eq Error)
if ($errors.Count) { $errors | ForEach-Object { Write-Host $_ }; throw 'Recorder C# typecheck failed.' }
Write-Host 'Recorder C# typecheck passed (no binary emitted).'
