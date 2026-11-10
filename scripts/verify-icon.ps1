# Verify that every icon image from assets/app.ico is embedded in a
# built codexbar-windows.exe. The build falls back to a warning when
# the resource compiler is unavailable or fails, so a pipeline that
# ships the program must check the result itself (v0.9.0 shipped
# without the icon because nothing did).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Ico = "assets/app.ico"
)

$icoBytes = [IO.File]::ReadAllBytes($Ico)
$count = [BitConverter]::ToUInt16($icoBytes, 4)
$exeBytes = [IO.File]::ReadAllBytes($Exe)
$enc = [Text.Encoding]::GetEncoding(28591)
$exeText = $enc.GetString($exeBytes)

$missing = 0
for ($i = 0; $i -lt $count; $i++) {
    $e = 6 + 16 * $i
    $size = [BitConverter]::ToUInt32($icoBytes, $e + 8)
    $off = [BitConverter]::ToUInt32($icoBytes, $e + 12)
    $img = $enc.GetString($icoBytes, $off, $size)
    if ($exeText.IndexOf($img, [StringComparison]::Ordinal) -lt 0) {
        $missing++
    }
}

if ($missing -gt 0) {
    Write-Error "Embedded app icon check FAILED: $missing of $count icon images are missing from $Exe."
    exit 1
}
Write-Host "Embedded app icon check OK: all $count icon images present in $Exe."
