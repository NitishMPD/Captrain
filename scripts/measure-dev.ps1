# Run alongside native verification, using the same fresh output directory.
param([string]$OutputDirectory = (Join-Path $PSScriptRoot '../verification'))
$ErrorActionPreference = 'Stop'
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
function Wait-Marker([string]$Name, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while (!(Test-Path -LiteralPath (Join-Path $OutputDirectory $Name))) {
        if ((Get-Date) -gt $deadline) { throw "Native check did not write $Name" }
        Start-Sleep -Milliseconds 250
    }
}
Wait-Marker 'idle-ready' 180
$captureProcess = Get-CimInstance Win32_Process -Filter "Name='captrain.exe'" | Select-Object -First 1
if (!$captureProcess) { throw 'Captrain is not running' }
function Get-CaptureTree {
    $rows = @(Get-CimInstance Win32_Process)
    $processIds = [Collections.Generic.HashSet[int]]::new()
    [void]$processIds.Add([int]$captureProcess.ProcessId)
    do {
        $added = $false
        foreach ($row in $rows) {
            if ($processIds.Contains([int]$row.ParentProcessId) -and $processIds.Add([int]$row.ProcessId)) { $added = $true }
        }
    } while ($added)
    @(foreach ($processId in $processIds) { Get-Process -Id $processId -ErrorAction SilentlyContinue })
}
$before = Get-CaptureTree
$cpuBefore = ($before | Measure-Object CPU -Sum).Sum
$timer = [Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds 60
$after = Get-CaptureTree
$elapsed = $timer.Elapsed.TotalSeconds
$cpu = 100 * (($after | Measure-Object CPU -Sum).Sum - $cpuBefore) / $elapsed
$report = [ordered]@{
    mode = 'debug'
    durationSeconds = $elapsed
    processes = $after.Count
    privateMB = ($after | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
    cpuOneCorePercent = $cpu
    cpuMachinePercent = $cpu / [Environment]::ProcessorCount
    logicalProcessors = [Environment]::ProcessorCount
    members = @($after | Select-Object ProcessName, Id, PrivateMemorySize64)
}
$report | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'resources.json')
Write-Output '60-second idle sample saved; waiting for 100 cycles.'
Wait-Marker 'cycles-end' 300
Start-Sleep -Seconds 3
$end = Get-CaptureTree
$report['afterCyclesPrivateMB'] = ($end | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
$report['afterCyclesProcesses'] = $end.Count
$report['afterCyclesMembers'] = @($end | Select-Object ProcessName, Id, PrivateMemorySize64)
$report | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'resources.json')
Write-Output ($report | ConvertTo-Json -Depth 5)
