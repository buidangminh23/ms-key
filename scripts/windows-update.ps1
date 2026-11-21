$ErrorActionPreference = 'Stop'
$targetDir = Split-Path -LiteralPath $targetExe
$backup = Join-Path $stage 'previous.exe'
$uninstaller = Join-Path $targetDir 'Uninstall.exe'
$oldUninstaller = Join-Path $stage 'previous-uninstaller.exe'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MSKey'
$oldVersion = $null
$runningChild = $null
$parentExited = $false
$changed = $false

try {
    $parent = Get-Process -Id $parentId -ErrorAction SilentlyContinue
    Copy-Item -LiteralPath $targetExe -Destination $backup
    if ($isInstaller) {
        if (Test-Path -LiteralPath $uninstaller) {
            Copy-Item -LiteralPath $uninstaller -Destination $oldUninstaller
        }
        $oldVersion = (Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction SilentlyContinue).DisplayVersion
    } else {
        $unpacked = Join-Path $stage 'unpacked'
        Expand-Archive -LiteralPath $package -DestinationPath $unpacked
        $newExe = Join-Path $unpacked 'MSKey.exe'
        if (!(Test-Path -LiteralPath $newExe -PathType Leaf)) { throw 'MSKey.exe missing from the archive.' }
    }
    [IO.File]::WriteAllText((Join-Path $stage 'ready'), 'ready')
    if ($parent -and !$parent.WaitForExit(30000)) { throw 'The previous MS Key process did not exit.' }
    $parentExited = $true
    $changed = $true
    if ($isInstaller) {
        $runningChild = Start-Process -FilePath $package -ArgumentList ('/S /D=' + $targetDir) -PassThru
        if (!$runningChild.WaitForExit(120000)) { throw 'The installer timed out.' }
        if ($runningChild.ExitCode -ne 0) { throw "Installer exit code $($runningChild.ExitCode)." }
    } else {
        Copy-Item -LiteralPath $newExe -Destination $targetExe -Force
        foreach ($name in 'LICENSE.txt','THIRD-PARTY-NOTICES.txt') {
            $file = Join-Path $unpacked $name
            if (Test-Path -LiteralPath $file) { Copy-Item -LiteralPath $file -Destination $targetDir -Force }
        }
    }
    $runningChild = Start-Process -FilePath $targetExe -ArgumentList '--show --no-relaunch' -PassThru
    if (!$runningChild.WaitForInputIdle(20000) -or $runningChild.WaitForExit(500)) {
        throw 'The updated app did not start its interface.'
    }
} catch {
    $reason = $_.Exception.Message
    try {
        if ($runningChild -and !$runningChild.HasExited) {
            $runningChild.Kill()
            if (!$runningChild.WaitForExit(5000)) { throw 'Could not stop the update process safely.' }
        }
        if ($changed) {
            Copy-Item -LiteralPath $backup -Destination $targetExe -Force
            if (Test-Path -LiteralPath $oldUninstaller) {
                Copy-Item -LiteralPath $oldUninstaller -Destination $uninstaller -Force
            }
            if ($isInstaller -and $oldVersion) {
                Set-ItemProperty -LiteralPath $uninstallKey -Name DisplayVersion -Value $oldVersion
            }
        }
        if ($parentExited) { Start-Process -FilePath $targetExe -ArgumentList '--show' }
    } catch { $reason += " Recovery: $($_.Exception.Message)" }
    [IO.File]::WriteAllText((Join-Path $stage 'update-error.txt'), $reason)
    if ($parentExited) {
        (New-Object -ComObject WScript.Shell).Popup("$reason Run MS Key's update check to retry. Backup: $backup", 20, 'MS Key', 48) | Out-Null
    }
    exit 1
}
Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
