<#
.SYNOPSIS
  测量 port-helper 发布版的内存：主进程 + 全部 WebView2 子进程（见 prd/todo/perf-todo.md P0-1）。

.DESCRIPTION
  依次测量三个场景：
    1. idle       启动后空闲 N 秒
    2. queried    输入端口查询，再按 F5 连续刷新 20 次后
    3. minimized  窗口最小化 30 秒后
  测量前必须关闭其他 port-helper 实例（开发版与发布版共用 WebView2 进程，会导致数据失真）。
  模拟键盘输入期间请不要操作鼠标键盘。

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/measure-memory.ps1 -Label before -Json prd/todo/perf-before.json
#>
param(
  [string]$Exe = "$PSScriptRoot\..\backend\target\release\port-helper.exe",
  [int]$IdleSeconds = 10,
  [int]$MinimizedSeconds = 30,
  [int]$Port = 5173,
  [string]$Label = "run",
  [string]$Json,
  # 追加的 WebView2 启动参数（通过 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS 传入，用于 A/B 试验）。
  # 注意：会覆盖 Tauri 的默认参数，所以脚本会自动带上默认的 --disable-features
  [string]$BrowserArgs
)

$ErrorActionPreference = "Stop"
if (Get-Process port-helper -ErrorAction SilentlyContinue) {
  throw "检测到正在运行的 port-helper，请先关闭（包括 pnpm dev 启动的开发版）。"
}
$Exe = (Resolve-Path $Exe).Path

Add-Type @"
using System; using System.Runtime.InteropServices;
public static class PhWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
"@
Add-Type -AssemblyName System.Windows.Forms

function Get-Tree([int]$RootId) {
  $all = Get-CimInstance Win32_Process
  $ids = [System.Collections.Generic.List[int]]::new(); $ids.Add($RootId)
  $queue = [System.Collections.Generic.Queue[int]]::new(); $queue.Enqueue($RootId)
  while ($queue.Count) {
    $parent = $queue.Dequeue()
    foreach ($c in $all | Where-Object { $_.ParentProcessId -eq $parent }) { $ids.Add([int]$c.ProcessId); $queue.Enqueue([int]$c.ProcessId) }
  }
  $perf = @{}
  Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | ForEach-Object { $perf[[int]$_.IDProcess] = $_ }
  foreach ($id in $ids) {
    $cim = $all | Where-Object { $_.ProcessId -eq $id }
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if (-not $p -or -not $cim) { continue }
    $type = if ($cim.CommandLine -match '--type=(\S+)') { $Matches[1] } elseif ($cim.Name -eq 'port-helper.exe') { 'app' } else { 'browser' }
    [pscustomobject]@{
      Process     = $cim.Name
      Type        = $type
      PrivateWSMB = [math]::Round(($perf[$id].WorkingSetPrivate) / 1MB, 1)   # 任务管理器“内存”列
      CommitMB    = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
      WorkingSetMB= [math]::Round($p.WorkingSet64 / 1MB, 1)
      Threads     = $p.Threads.Count
    }
  }
}

function Measure-Scenario([string]$Name, [int]$RootId) {
  $rows = @(Get-Tree $RootId)
  $sum = [pscustomobject]@{
    Scenario    = $Name
    Processes   = $rows.Count
    AppPrivateWS= ($rows | Where-Object Type -eq 'app' | Measure-Object PrivateWSMB -Sum).Sum
    PrivateWSMB = [math]::Round(($rows | Measure-Object PrivateWSMB -Sum).Sum, 1)
    CommitMB    = [math]::Round(($rows | Measure-Object CommitMB -Sum).Sum, 1)
    AppThreads  = ($rows | Where-Object Type -eq 'app').Threads
  }
  Write-Host "`n== $Label / $Name ==" -ForegroundColor Cyan
  $rows | Sort-Object Type | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
  Write-Host ("合计：私有工作集 {0} MB，提交 {1} MB，进程 {2} 个" -f $sum.PrivateWSMB, $sum.CommitMB, $sum.Processes)
  return [pscustomobject]@{ summary = $sum; detail = $rows }
}

$defaultArgs = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection"
if ($BrowserArgs) { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "$defaultArgs $BrowserArgs" }
else { Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue }

function Get-TreeCpuSeconds([int]$RootId) {
  $ids = @($RootId) + @((Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $RootId }).ProcessId)
  $all = Get-CimInstance Win32_Process
  $queue = [System.Collections.Generic.Queue[int]]::new(); $queue.Enqueue($RootId); $set = @{ $RootId = 1 }
  while ($queue.Count) { $pp = $queue.Dequeue(); foreach ($c in $all | Where-Object { $_.ParentProcessId -eq $pp }) { if (-not $set[[int]$c.ProcessId]) { $set[[int]$c.ProcessId] = 1; $queue.Enqueue([int]$c.ProcessId) } } }
  ($set.Keys | ForEach-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).TotalProcessorTime.TotalSeconds } | Measure-Object -Sum).Sum
}

$proc = Start-Process -FilePath $Exe -PassThru
try {
  Start-Sleep -Seconds $IdleSeconds
  $proc.Refresh()
  $hwnd = $proc.MainWindowHandle
  $results = @()
  $results += Measure-Scenario "idle" $proc.Id

  [PhWin]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 500
  $cpuBefore = Get-TreeCpuSeconds $proc.Id
  [System.Windows.Forms.SendKeys]::SendWait("$Port{ENTER}")
  Start-Sleep -Seconds 3
  for ($i = 0; $i -lt 20; $i++) {
    [PhWin]::SetForegroundWindow($hwnd) | Out-Null
    [System.Windows.Forms.SendKeys]::SendWait("{F5}")
    Start-Sleep -Milliseconds 2600   # 等待刷新动画完整结束（2 轮柔光约 2.2s）
  }
  Start-Sleep -Seconds 2
  $cpuQueried = [math]::Round((Get-TreeCpuSeconds $proc.Id) - $cpuBefore, 2)
  $results += Measure-Scenario "queried" $proc.Id
  $results[-1].summary | Add-Member -NotePropertyName CpuSeconds -NotePropertyValue $cpuQueried
  Write-Host "查询 + 20 次刷新期间 CPU 时间：$cpuQueried 秒（含刷新动画）"

  [PhWin]::ShowWindow($hwnd, 6) | Out-Null   # SW_MINIMIZE
  Start-Sleep -Seconds $MinimizedSeconds
  $results += Measure-Scenario "minimized" $proc.Id
}
finally {
  Stop-Process -Id $proc.Id -ErrorAction SilentlyContinue
}

Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
Write-Host "`n== 汇总（$Label）==" -ForegroundColor Green
$results.summary | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
if ($Json) {
  $results | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 $Json
  Write-Host "已写入 $Json"
}
