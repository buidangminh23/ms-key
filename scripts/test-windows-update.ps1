#requires -Version 5.1
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -ne 'Desktop') { throw 'Run with Windows PowerShell 5.1.' }

$tempDir = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$root = Join-Path $tempDir ('mskey-update-test-' + [guid]::NewGuid())
$helper = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'windows-update.ps1'))
$fixtureSource = @'
using System;
using System.Diagnostics;
using System.IO;
using System.Threading;
using System.Windows.Forms;

static class Fixture {
    [STAThread]
    static int Main(string[] args) {
        string dir = AppDomain.CurrentDomain.BaseDirectory;
        if (Array.IndexOf(args, "/S") >= 0) {
            string install = string.Join(" ", args).Split(new string[] {"/D="}, StringSplitOptions.None)[1];
            File.Copy(Path.Combine(dir, "replacement.exe"), Path.Combine(install, "MSKey.exe"), true);
            File.WriteAllText(Path.Combine(install, "Uninstall.exe"), "new uninstaller");
            File.WriteAllText(Path.Combine(install, "version.txt"), "new");
            File.WriteAllText(Path.Combine(install, "installer.pid"), Process.GetCurrentProcess().Id.ToString());
            if (File.Exists(Path.Combine(install, "hang"))) Thread.Sleep(60000);
            return 0;
        }
#if PREVIOUS
        string name = args.Length == 0 ? "parent" : "restored";
#else
        string name = "updated";
#endif
        File.WriteAllText(Path.Combine(dir, name + ".pid"), Process.GetCurrentProcess().Id.ToString());
        if (name == "updated" && File.Exists(Path.Combine(dir, "reject"))) return 1;
        Application.Run(new ApplicationContext());
        return 0;
    }
}
'@
$fixturePreamble = @'
param([string]$caseDir, [int]$parentId)
$ErrorActionPreference = 'Stop'
$stage = Join-Path $caseDir 'stage'
$targetDir = Join-Path $caseDir 'app folder'
$targetExe = Join-Path $targetDir 'MSKey.exe'
$isInstaller = Test-Path -LiteralPath (Join-Path $stage 'setup.exe')
$package = if ($isInstaller) { Join-Path $stage 'setup.exe' } else { Join-Path $stage 'package.zip' }
function Get-ItemProperty($LiteralPath, $ErrorAction) {
    if ($LiteralPath -ne $uninstallKey) { throw "Unexpected registry read: $LiteralPath" }
    @{ DisplayVersion = [IO.File]::ReadAllText((Join-Path $targetDir 'version.txt')) }
}
function Set-ItemProperty($LiteralPath, $Name, $Value) {
    if ($LiteralPath -ne $uninstallKey -or $Name -ne 'DisplayVersion') { throw 'Unexpected registry write.' }
    [IO.File]::WriteAllText((Join-Path $targetDir 'version.txt'), $Value)
}
function New-FixturePopup {
    Add-Member -InputObject ([PSCustomObject]@{}) -MemberType ScriptMethod -Name Popup -Value { 1 } -PassThru
}
'@

function Assert-True([bool]$condition, [string]$message) {
    if (!$condition) { throw $message }
}

function Wait-ForFile([string]$path, [Diagnostics.Process]$whileRunning) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (!(Test-Path -LiteralPath $path)) {
        if ($timer.Elapsed.TotalSeconds -gt 15 -or ($whileRunning -and $whileRunning.HasExited)) {
            throw "Missing fixture marker: $path"
        }
        Start-Sleep -Milliseconds 50
    }
}

function Test-UpdateCase([string]$name) {
    $caseDir = Join-Path $root $name
    $app = Join-Path $caseDir 'app folder'
    $stage = Join-Path $caseDir 'stage'
    New-Item -ItemType Directory -Path $app,$stage | Out-Null
    $exe = Join-Path $app 'MSKey.exe'
    Copy-Item -LiteralPath $previous -Destination $exe
    $installer = $name -like '*installer'
    if ($installer) {
        Copy-Item -LiteralPath $previous -Destination (Join-Path $stage 'setup.exe')
        Copy-Item -LiteralPath $replacement -Destination (Join-Path $stage 'replacement.exe')
        [IO.File]::WriteAllText((Join-Path $app 'Uninstall.exe'), 'previous uninstaller')
        [IO.File]::WriteAllText((Join-Path $app 'version.txt'), 'previous')
        if ($name -eq 'hung installer') { [IO.File]::WriteAllText((Join-Path $app 'hang'), '') }
    } else {
        Compress-Archive -LiteralPath $replacement -DestinationPath (Join-Path $stage 'package.zip')
        if ($name -eq 'rejected startup') { [IO.File]::WriteAllText((Join-Path $app 'reject'), '') }
    }
    $parent = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
    $runner = $null
    try {
        Wait-ForFile (Join-Path $app 'parent.pid') $parent
        $argsLine = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" "{1}" {2}' -f $scriptPath,$caseDir,$parent.Id
        $stderr = Join-Path $caseDir 'stderr.txt'
        $runner = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList $argsLine -WindowStyle Hidden -PassThru -RedirectStandardError $stderr
        $null = $runner.Handle
        try {
            Wait-ForFile (Join-Path $stage 'ready') $runner
        } catch {
            foreach ($log in $stderr,(Join-Path $stage 'update-error.txt')) {
                if (Test-Path -LiteralPath $log) { Write-Host ([IO.File]::ReadAllText($log)) }
            }
            throw
        }
        $parent.Kill()
        Assert-True ($runner.WaitForExit(25000)) "$name did not finish."
        if ($name -like 'success*') {
            if ($runner.ExitCode -ne 0) {
                $diagnostic = Get-Content -LiteralPath (Join-Path $stage 'update-error.txt') -Raw -ErrorAction SilentlyContinue
                throw "The update exited with code [$($runner.ExitCode)]. $diagnostic"
            }
            Assert-True (!(Test-Path -LiteralPath $stage)) 'The completed update left its stage behind.'
            Wait-ForFile (Join-Path $app 'updated.pid') $null
            $expectedExe = $replacement
        } else {
            Assert-True ($runner.ExitCode -eq 1) "$name did not report failure."
            $reason = [IO.File]::ReadAllText((Join-Path $stage 'update-error.txt'))
            Assert-True ($reason -notmatch 'Recovery:') "Recovery failed: $reason"
            if ($installer) {
                Assert-True ($reason -match 'installer timed out') "Unexpected error: $reason"
                $installerPid = [int][IO.File]::ReadAllText((Join-Path $app 'installer.pid'))
                $alive = Get-Process -Id $installerPid -ErrorAction SilentlyContinue
                Assert-True (!$alive -or $alive.Path -ne (Join-Path $stage 'setup.exe')) 'Timed-out installer is still running.'
                Assert-True ([IO.File]::ReadAllText((Join-Path $app 'Uninstall.exe')) -ceq 'previous uninstaller') 'Uninstaller was not restored.'
                Assert-True ([IO.File]::ReadAllText((Join-Path $app 'version.txt')) -ceq 'previous') 'Registry version was not restored.'
            } else {
                Assert-True (Test-Path -LiteralPath (Join-Path $app 'updated.pid')) 'Replacement was never launched.'
            }
            Wait-ForFile (Join-Path $app 'restored.pid') $null
            $expectedExe = $previous
        }
        Assert-True ((Get-FileHash -LiteralPath $exe).Hash -eq (Get-FileHash -LiteralPath $expectedExe).Hash) "Incorrect installed executable after $name."
        Write-Host "Passed: $name"
    } finally {
        if ($runner -and !$runner.HasExited) { $runner.Kill(); $runner.WaitForExit() }
        foreach ($process in (Get-Process -Name MSKey,setup -ErrorAction SilentlyContinue)) {
            if ($process.Path -in @($exe, (Join-Path $stage 'setup.exe')) -and !$process.HasExited) {
                $process.Kill()
                $process.WaitForExit()
            }
        }
    }
}

New-Item -ItemType Directory -Path $root | Out-Null
try {
    $source = Join-Path $root 'Fixture.cs'
    $previous = Join-Path $root 'previous.exe'
    $replacement = Join-Path $root 'MSKey.exe'
    $scriptPath = Join-Path $root 'run-helper.ps1'
    [IO.File]::WriteAllText($source, $fixtureSource)
    $compiler = Join-Path ([Runtime.InteropServices.RuntimeEnvironment]::GetRuntimeDirectory()) 'csc.exe'
    foreach ($output in $previous,$replacement) {
        $symbol = if ($output -eq $previous) { 'PREVIOUS' } else { 'UPDATED' }
        & $compiler /nologo /target:winexe /r:System.Windows.Forms.dll "/define:$symbol" "/out:$output" $source
        if ($LASTEXITCODE) { throw 'Could not compile the updater fixture.' }
    }
    Assert-True ($helper.Contains('WaitForExit(120000)')) 'Update the test override for the installer deadline.'
    $helper = $helper.Replace('New-Object -ComObject WScript.Shell', 'New-FixturePopup')
    $runScript = $fixturePreamble + [Environment]::NewLine + $helper.Replace('WaitForExit(120000)', 'WaitForExit(1500)')
    [IO.File]::WriteAllText($scriptPath, $runScript, [Text.Encoding]::UTF8)
    foreach ($name in 'success archive','success installer','hung installer','rejected startup') { Test-UpdateCase $name }
} finally {
    $resolved = (Resolve-Path -LiteralPath $root).ProviderPath
    if ([IO.Path]::GetFullPath($root) -ne $resolved -or [IO.Path]::GetDirectoryName($resolved) -ne $tempDir.TrimEnd('\')) {
        throw "Unexpected fixture path: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
