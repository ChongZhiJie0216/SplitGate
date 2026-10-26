# Run sing-box with its config passed on STDIN, so configs holding passwords or keys never touch the disk.
# Dot-source this file, then:
#   Invoke-SingBoxStdin -Mode check -ConfigJson $json            -> returns exit code
#   Invoke-SingBoxStdin -Mode run -ConfigJson $json -LogFile x   -> streams output to console + log, returns exit code
# Uses System.Diagnostics.Process directly: PowerShell pipes add a BOM, and "2>&1" on native exes
# turns stderr lines into error records in Windows PowerShell 5.1.
function Invoke-SingBoxStdin {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('check', 'run')][string]$Mode,
        [Parameter(Mandatory = $true)][string]$ConfigJson,
        [string]$LogFile = ''
    )
    $exe = Join-Path $BinDir 'sing-box\sing-box.exe'
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = "$Mode --disable-color -c stdin"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [Text.Encoding]::UTF8

    $p = New-Object Diagnostics.Process
    $p.StartInfo = $psi
    # .NET Framework builds the stdin writer from Console.InputEncoding and writes its BOM on start;
    # use a BOM-less UTF-8 just for this start (sing-box rejects a leading BOM).
    $savedInputEncoding = [Console]::InputEncoding
    try {
        [Console]::InputEncoding = New-Object Text.UTF8Encoding $false
        [void]$p.Start()
    } finally {
        [Console]::InputEncoding = $savedInputEncoding
    }

    # Write the config without BOM, then close stdin so sing-box starts parsing
    $bytes = (New-Object Text.UTF8Encoding $false).GetBytes($ConfigJson)
    $p.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    $p.StandardInput.Close()
    $ConfigJson = $null

    $writer = $null
    if ($LogFile) { $writer = New-Object IO.StreamWriter($LogFile, $true, (New-Object Text.UTF8Encoding $false)); $writer.AutoFlush = $true }
    try {
        # sing-box logs to stderr; stdout is normally empty. Read stderr line by line, drain stdout at the end.
        while (-not $p.StandardError.EndOfStream) {
            $line = $p.StandardError.ReadLine()
            Write-Host $line
            if ($writer) { $writer.WriteLine($line) }
        }
        $rest = $p.StandardOutput.ReadToEnd()
        if ($rest) { Write-Host $rest; if ($writer) { $writer.Write($rest) } }
        $p.WaitForExit()
        return $p.ExitCode
    } finally {
        if ($writer) { $writer.Dispose() }
        if (-not $p.HasExited) { try { $p.Kill() } catch {} }
    }
}
