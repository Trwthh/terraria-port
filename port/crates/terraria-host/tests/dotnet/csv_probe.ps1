# Expected values for the CsvHelper tests of terraria-host: the game's own CsvHelper.dll (version 2.0, meta\refs).
# The game uses it in one place: LanguageManager.LoadLanguageFromFileTextCsv (HasHeaderRecord, ReadHeader, FieldHeaders, Read, CurrentRecord).
# Usage (from this folder): powershell -NoProfile -ExecutionPolicy Bypass -File csv_probe.ps1
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..\..')
Add-Type -Path (Join-Path $root 'meta\refs\CsvHelper.dll')
$utf8 = New-Object System.Text.UTF8Encoding($false)
function B64($s) { [Convert]::ToBase64String($utf8.GetBytes($s)) }
function Esc($s) { if ($null -eq $s) { return "<null>" }; $s.Replace("`r", '\r').Replace("`n", '\n') }
function Run($text) {
    $out = ''
    try {
        $r = New-Object CsvHelper.CsvReader((New-Object IO.StringReader($text)))
        $r.Configuration.HasHeaderRecord = $true
        $h = $r.ReadHeader()
        $out = "header=$h fields=" + (($r.FieldHeaders | ForEach-Object { '[' + (Esc $_) + ']' }) -join '')
        $n = 0
        while ($r.Read()) { $n++; $out += " | rec$n(" + $r.CurrentRecord.Length + '):' + (($r.CurrentRecord | ForEach-Object { '[' + (Esc $_) + ']' }) -join '') }
    } catch { $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }; $out += ' ERR ' + $e.GetType().Name + ': ' + $e.Message }
    $out
}
$cases = [ordered]@{
    'plain' = "key,translation`nA,B`nC,D"
    'quotes-and-blank-line' = "Key,Translation,Notes`r`nA,`"hello, world`",x`r`n`r`nB,`"say `"`"hi`"`"`",y`r`n"
    'multiline-field' = "key,translation`n`"multi`nline`",v`n,onlytrans`nlast,v"
    'empty' = ''
    'newline-only' = "`n"
    'ragged' = "key,translation`nA`nB,C,D,E`n"
    'semicolon' = "key;translation`nA;B"
    'spaces' = "key , translation`n a , b "
    'cr-only' = "key,translation`r`rA,B"
    'unterminated-quote' = "key,translation`nA,`"unterminated"
    'header-only' = "key,translation"
    'header-only-newline' = "key,translation`r`n"
    'trailing-comma' = "key,translation,`nA,B,"
    'quote-in-middle' = "key,translation`nA,b`"c`nD,E"
    'empty-fields' = "key,translation`n,`n,,`nA,"
    'crlf-in-quotes' = "key,translation`r`nA,`"x`r`ny`"`r`nB,C"
    'whitespace-line' = "key,translation`n   `nA,B"
    'quoted-header' = "`"key`",`"translation`"`nA,B"
}
$lines = foreach ($k in $cases.Keys) { ($k, (B64 $cases[$k]), (B64 (Run $cases[$k]))) -join "`t" }
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'csv.expected'), (($lines -join "`n") + "`n"), $utf8)
"cases: $($cases.Count)"
