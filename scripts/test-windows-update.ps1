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
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

static class Fixture {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct WindowClass {
        public uint style;
        public IntPtr procedure;
        public int extraClassBytes, extraWindowBytes;
        public IntPtr instance, icon, cursor, background;
        public IntPtr menu, name;
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool GetClassInfo(IntPtr instance, string name, out WindowClass info);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern ushort RegisterClass(ref WindowClass info);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr CreateWindowEx(uint exStyle, string name, string title, uint style,
        int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
    [DllImport("user32.dll", EntryPoint = "PeekMessageW")]
    static extern bool PeekMessage(IntPtr message, IntPtr window, uint first, uint last, uint flags);
    [DllImport("user32.dll", EntryPoint = "DispatchMessageW")]
    static extern IntPtr DispatchMessage(IntPtr message);

    static void RunUpdatedWindow(string dir) {
        WindowClass windowClass;
        if (!GetClassInfo(IntPtr.Zero, "STATIC", out windowClass)) throw new Exception("Missing STATIC class.");
        windowClass.instance = Process.GetCurrentProcess().MainModule.BaseAddress;
        string name = File.Exists(Path.Combine(dir, "wrong class")) ? "MSKeyPlaceholder" : "MSKeyMainWindow";
        windowClass.name = Marshal.StringToHGlobalUni(name);
        try {
            if (RegisterClass(ref windowClass) == 0) throw new Exception("Could not register fixture class.");
        } finally { Marshal.FreeHGlobal(windowClass.name); }
        if (CreateWindowEx(0, name, "", 0x80000000, 0, 0, 1, 1,
            IntPtr.Zero, IntPtr.Zero, windowClass.instance, IntPtr.Zero) == IntPtr.Zero) {
            throw new Exception("Could not create fixture window.");
        }
        File.WriteAllText(Path.Combine(dir, "window-created.pid"), Process.GetCurrentProcess().Id.ToString());
        if (File.Exists(Path.Combine(dir, "hung window"))) Thread.Sleep(60000);
        const uint PM_REMOVE = 1, PM_NOYIELD = 2;
        IntPtr message = Marshal.AllocHGlobal(64);
        try {
            while (true) {
                while (PeekMessage(message, IntPtr.Zero, 0, 0, PM_REMOVE | PM_NOYIELD)) DispatchMessage(message);
                Thread.Sleep(5);
            }
        } finally { Marshal.FreeHGlobal(message); }
    }

    [STAThread]
    static int Main(string[] args) {
        string dir = AppDomain.CurrentDomain.BaseDirectory;
        if (Array.IndexOf(args, "--decoy") >= 0) { RunUpdatedWindow(dir); return 0; }
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
        if (name == "updated") RunUpdatedWindow(dir);
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
        if ($name -in 'hung window','wrong class') { [IO.File]::WriteAllText((Join-Path $app $name), '') }
    }
    $parent = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
    $runner = $null
    $decoy = $null
    try {
        Wait-ForFile (Join-Path $app 'parent.pid') $parent
        if ($name -eq 'wrong class') {
            $decoy = Start-Process -FilePath $previous -ArgumentList '--decoy' -WindowStyle Hidden -PassThru
            Wait-ForFile (Join-Path $root 'window-created.pid') $decoy
        }
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
            $updated = Get-Process -Id ([int][IO.File]::ReadAllText((Join-Path $app 'updated.pid')))
            Assert-True (!$updated.WaitForInputIdle(200)) 'The responsive fixture unexpectedly signaled input idle.'
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
                if ($name -in 'hung window','wrong class') {
                    Assert-True ($reason -match 'window did not respond') "Unexpected window error: $reason"
                }
            }
            Wait-ForFile (Join-Path $app 'restored.pid') $null
            $expectedExe = $previous
        }
        Assert-True ((Get-FileHash -LiteralPath $exe).Hash -eq (Get-FileHash -LiteralPath $expectedExe).Hash) "Incorrect installed executable after $name."
        Write-Host "Passed: $name"
    } finally {
        if ($decoy -and !$decoy.HasExited) { $decoy.Kill(); $decoy.WaitForExit() }
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
    Assert-True ($helper.Contains('ElapsedMilliseconds -ge 20000')) 'Update the test override for the startup deadline.'
    $helper = $helper.Replace('ElapsedMilliseconds -ge 20000', 'ElapsedMilliseconds -ge 1500')
    $helper = $helper.Replace('New-Object -ComObject WScript.Shell', 'New-FixturePopup')
    $runScript = $fixturePreamble + [Environment]::NewLine + $helper.Replace('WaitForExit(120000)', 'WaitForExit(1500)')
    [IO.File]::WriteAllText($scriptPath, $runScript, [Text.Encoding]::UTF8)
    foreach ($name in 'success archive','success installer','hung installer','rejected startup','hung window','wrong class') {
        Test-UpdateCase $name
    }
} finally {
    $resolved = (Resolve-Path -LiteralPath $root).ProviderPath
    if ([IO.Path]::GetFullPath($root) -ne $resolved -or [IO.Path]::GetDirectoryName($resolved) -ne $tempDir.TrimEnd('\')) {
        throw "Unexpected fixture path: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
