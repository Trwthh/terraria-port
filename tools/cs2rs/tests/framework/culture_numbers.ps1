# Number formats of the game's twelve cultures (GameCulture.cs) and the invariant culture, as the real .NET
# Framework prints them on this machine. Run with Windows PowerShell 5.1 (.NET Framework 4.8):
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\cs2rs\tests\framework\culture_numbers.ps1
#
# Writes, next to this script:
#   culture_numbers.tsv      one line per culture: the NumberFormatInfo members rt::fmt needs. Strings as UTF-16 code
#                            units in hex (4 digits each), so that no-break spaces and the like are unambiguous.
#   culture_fmt_probe.out    expected texts: culture <TAB> kind (f32 | f64 | i32) <TAB> value (bit pattern as an
#                            unsigned decimal number for floats, the number for i32) <TAB> format ("-" = ToString())
#                            <TAB> the text, as UTF-16 code units in hex
# The unit test `rt::fmt::tests::culture_formats_match_dotnet` replays every line.
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$names = @('', 'en-US', 'de-DE', 'it-IT', 'fr-FR', 'es-ES', 'ru-RU', 'zh-Hans', 'pt-BR', 'pl-PL', 'ja-JP', 'ko-KR', 'zh-Hant')
function Hex([string]$s) {
    $sb = New-Object System.Text.StringBuilder
    foreach ($c in $s.ToCharArray()) { [void]$sb.Append(([int]$c).ToString('x4')) }
    if ($sb.Length -eq 0) { return '-' }
    return $sb.ToString()
}
$enc = New-Object System.Text.UTF8Encoding($false)
$rows = New-Object System.Collections.Generic.List[string]
$rows.Add("# name`tNumberDecimalSeparator`tNumberGroupSeparator`tNumberGroupSizes`tNegativeSign`tPositiveSign`tNumberNegativePattern`tNumberDecimalDigits`tPercentDecimalSeparator`tPercentGroupSeparator`tPercentGroupSizes`tPercentSymbol`tPercentPositivePattern`tPercentNegativePattern`tPercentDecimalDigits`tNaNSymbol`tPositiveInfinitySymbol`tNegativeInfinitySymbol`tPerMilleSymbol")
$probe = New-Object System.Collections.Generic.List[string]
$floats = @(0.0, 1.0, -1.0, 0.5, 1.5, -2.25, 1234.5678, 1234567.891, 0.001, 12.345, 100000.0, 0.00001, 15000000000.0, 1e20, 0.125, -0.5, 0.999, 99.5, 1e-10, 3.14159274)
$special = @([double]::NaN, [double]::PositiveInfinity, [double]::NegativeInfinity)
$ints = @(0, 7, -7, 1234, -1234567, 1000000, 2147483647)
$ffmt = @('-', 'F2', 'F0', 'N', 'N0', 'N2', 'P', 'P0', 'P1', '0.0', '0', '00', '0.0%', '#,##0.00', '+0;-#', '0.00', '#.##')
$ifmt = @('-', 'D', 'D4', 'X2', 'N0', 'N', 'F2', 'P0', '+0;-#', '00', '#,##0')
foreach ($n in $names) {
    $c = if ($n -eq '') { [System.Globalization.CultureInfo]::InvariantCulture } else { [System.Globalization.CultureInfo]::GetCultureInfo($n) }
    $f = $c.NumberFormat
    $label = if ($n -eq '') { 'invariant' } else { $n }
    $rows.Add(($label, (Hex $f.NumberDecimalSeparator), (Hex $f.NumberGroupSeparator), ($f.NumberGroupSizes -join ','), (Hex $f.NegativeSign), (Hex $f.PositiveSign), $f.NumberNegativePattern, $f.NumberDecimalDigits,
        (Hex $f.PercentDecimalSeparator), (Hex $f.PercentGroupSeparator), ($f.PercentGroupSizes -join ','), (Hex $f.PercentSymbol), $f.PercentPositivePattern, $f.PercentNegativePattern, $f.PercentDecimalDigits,
        (Hex $f.NaNSymbol), (Hex $f.PositiveInfinitySymbol), (Hex $f.NegativeInfinitySymbol), (Hex $f.PerMilleSymbol)) -join "`t")
    foreach ($v in $floats + $special) {
        $d = [double]$v
        $s = [float]$v
        $db = [BitConverter]::ToUInt64([BitConverter]::GetBytes($d), 0)
        $sb = [BitConverter]::ToUInt32([BitConverter]::GetBytes($s), 0)
        foreach ($fm in $ffmt) {
            if ($fm -eq '-') { $td = $d.ToString($c); $ts = $s.ToString($c) } else { $td = $d.ToString($fm, $c); $ts = $s.ToString($fm, $c) }
            $probe.Add(($label, 'f64', $db, $fm, (Hex $td)) -join "`t")
            $probe.Add(($label, 'f32', $sb, $fm, (Hex $ts)) -join "`t")
        }
    }
    foreach ($v in $ints) {
        $i = [int]$v
        foreach ($fm in $ifmt) {
            if ($fm -eq '-') { $t = $i.ToString($c) } else { $t = $i.ToString($fm, $c) }
            $probe.Add(($label, 'i32', $i, $fm, (Hex $t)) -join "`t")
        }
    }
}
[System.IO.File]::WriteAllLines((Join-Path $here 'culture_numbers.tsv'), $rows, $enc)
[System.IO.File]::WriteAllLines((Join-Path $here 'culture_fmt_probe.out'), $probe, $enc)
"cultures: $($names.Count); probe lines: $($probe.Count); CLR $([System.Environment]::Version)"
