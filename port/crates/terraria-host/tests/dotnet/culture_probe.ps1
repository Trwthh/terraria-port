# Expected values for the DateTime formatting tests of terraria-host: real .NET Framework, every culture the game supports.
# Usage (from this folder): powershell -NoProfile -ExecutionPolicy Bypass -File culture_probe.ps1
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
function B64($s) { [Convert]::ToBase64String($utf8.GetBytes($s)) }
$cultures = '', 'en-US', 'de-DE', 'it-IT', 'fr-FR', 'es-ES', 'ru-RU', 'zh-Hans', 'pt-BR', 'pl-PL', 'ja-JP', 'ko-KR', 'zh-Hant'
$dates = @(
    (New-Object DateTime(2026, 10, 11, 5, 17, 42)).AddTicks(1234567),
    (New-Object DateTime(1999, 12, 31, 23, 59, 59)),
    (New-Object DateTime(2000, 3, 1, 0, 0, 0)),
    (New-Object DateTime(2024, 2, 29, 12, 0, 0)),
    (New-Object DateTime(2021, 7, 4, 15, 4, 5))
)
$formats = 'D', 'd', 'MMMM d, yyy', 'd MMMM yyyy', 'MM-dd-yy_HH-mm-ss-ffff', 'dddd', 'ddd', 'MMM', 'MMMM', 'yyyy/MM/dd', 'H:mm:ss', 'hh:mm tt', 'd/M/yy', 'dddd, MMMM d, yyyy', 'D4', 'MMMM yyyy', 'yyyy''-''MM', 'h:m:s t'
$lines = New-Object System.Collections.Generic.List[string]
foreach ($cn in $cultures) {
    $c = [Globalization.CultureInfo]::GetCultureInfo($cn)
    [Threading.Thread]::CurrentThread.CurrentCulture = $c
    foreach ($d in $dates) {
        foreach ($f in $formats) {
            if ($f -eq 'D4') { continue }
            $r = try { $d.ToString($f) } catch { 'ERR' }
            $lines.Add(($cn, $d.Ticks, $f, (B64 $r)) -join "`t")
        }
        $lines.Add(($cn, $d.Ticks, 'ToShortDateString', (B64 $d.ToShortDateString())) -join "`t")
        $lines.Add(($cn, $d.Ticks, 'ToLongDateString', (B64 $d.ToLongDateString())) -join "`t")
    }
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'datetime.expected'), (($lines -join "`n") + "`n"), $utf8)
"lines: $($lines.Count)"
