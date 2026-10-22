# Expected values for the Newtonsoft.Json tests of terraria-host: the game's own Newtonsoft.Json.dll (version 10) driven from Windows PowerShell 5.1.
# Usage (from this folder): powershell -NoProfile -ExecutionPolicy Bypass -File json_probe.ps1
# Writes json_cases.expected (edge cases: input and output in base64) and json_corpus.expected (SHA-256 of the output for every embedded JSON resource).
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..\..')
Add-Type -Path (Join-Path $root 'meta\refs\Newtonsoft.Json.dll')
$utf8 = New-Object System.Text.UTF8Encoding($false)
function B64($s) { [Convert]::ToBase64String($utf8.GetBytes($s)) }
function Sha($s) { $h = [System.Security.Cryptography.SHA256]::Create(); ([BitConverter]::ToString($h.ComputeHash($utf8.GetBytes($s)))).Replace('-', '').ToLower() }
function Run($f) { try { 'OK:' + (& $f) } catch { $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }; 'ERR:' + $e.GetType().Name + ': ' + $e.Message } }
$J = [Newtonsoft.Json.Linq.JToken]
$cases = [ordered]@{}
function C($name, $json) { $script:cases[$name] = $json }
C 'trailing-comma-array' '[1,2,]'
C 'trailing-comma-object' '{"a":1,}'
C 'leading-comma-array' '[,1]'
C 'double-comma-array' '[1,,2]'
C 'single-quote-unquoted-names' "{a:1,'b':2,`"c d`":3}"
C 'extra-text' '[1,2] x'
C 'comments' "/*c*/ {`"a`":/*d*/1 //e`n}"
C 'comments-in-array' "[1,/*x*/2,//y`n3]"
C 'comments-in-object' "{/*a*/`"p`":/*b*/1,/*c*/`"q`":2/*d*/}"
C 'numbers' '{"a":[],"b":{},"c":1.0,"d":1e20,"e":12345678901234567890,"f":1E5,"g":0.1,"h":-0.0,"i":123456789012345678,"j":-9223372036854775808,"k":9223372036854775808,"l":1.5e-7,"m":100000000000000000000000,"n":1e400}'
C 'floats' '[0.30000000000000004,5e-324,1.7976931348623157e308,123456789012345.6,1e15,1e14,0.00001,0.000001,1234567.125,3.14159265358979,0.1,100,1e-5,12345678901234567.0,2.5E+3]'
C 'dup-keys' '{"a":1,"b":2,"a":3}'
C 'escapes' @'
"a\"b\\c\/d\b\f\n\r\t\u0001\u001f\u007f\u0085\u2028\u2029\u00e9\ud83d\ude00 \u0041\'q"
'@
C 'escapes-single-quoted' @'
['it\'s "q"', 'a\u0027b', "x\ty"]
'@
C 'escape-names' @'
{"a\"b":1,"c\\d":2,"e\nf":3,"\u2028":4,"\u00e9":5,"\u0000":6}
'@
C 'raw-controls' "`"tab`there nl`n cr`r`""
C 'nan-inf' '[NaN,Infinity,-Infinity,undefined,null,true,false]'
C 'leading-zero' '[01,-01,00.5]'
C 'hex' '[0x1F]'
C 'missing-colon' '{"a" 1}'
C 'unterminated-string' '"abc'
C 'unterminated-object' '{"a":1'
C 'unterminated-array' '[1,2'
C 'bad-value' '{"a":@}'
C 'bad-escape' '"\q"'
C 'empty' ''
C 'whitespace' '   '
C 'scalar-string' '"hello"'
C 'scalar-int' '42'
C 'scalar-neg' '-7'
C 'scalar-true' 'true'
C 'scalar-null' 'null'
C 'dates' '["2020-01-01T10:11:12Z","2020-01-01T10:11:12","2020-01-01T10:11:12.5","2020-01-01T10:11:12.1234567Z","2020-02-30T10:11:12Z","2020-01-01","2020-01-01 10:11:12","/Date(1577873472000)/","/Date(0)/"]'
C 'dates-ms-offset' '["/Date(1577873472000)/","/Date(-1)/","/Date(1577873472123)/"]'
C 'dates-fraction' '["2020-01-01T10:11:12.0000001","2020-01-01T10:11:12.000","2020-01-01T10:11:12.9999999Z","0001-01-01T00:00:00","9999-12-31T23:59:59.9999999Z","2020-01-01T10:11"]'
C 'big-ints' '[0,-0,00,9223372036854775807,-9223372036854775808,9223372036854775808,-9223372036854775809,123456789012345678901234567890]'
C 'strings-numeric' '["1","1.0","true","null","2020","-"]'
C 'nested' '{"a":{"b":{"c":[1,[2,[3,{}]]]}},"d":[{"e":null},[]]}'
C 'unicode-keys' ('{"' + [char]0xE9 + '":"' + [char]0xFC + '","' + [char]0x65E5 + [char]0x672C + '":"' + [char]0x8A9E + '","a
b":"	"}')
C 'bom-text' ([string][char]0xFEFF + '{"a":1}')
C 'big-exponent' '[1E+2,1e+2,1E2,1.0E2,-1e-2]'
C 'trailing-garbage-after-scalar' '1 2'
C 'colon-in-array' '[1:2]'
C 'constructor' 'new Date(1,2)'
C 'unquoted-value' '{"a":abc}'
C 'long-string' ('"' + ('x' * 5000) + '"')
C 'deep' (('[' * 100) + (']' * 100))
C 'spaces-in-numbers' '[1 2]'
C 'minus-only' '[-]'
C 'plus' '[+1]'
C 'dot' '[.5]'
C 'trailing-dot' '[1.]'
$out = New-Object System.Collections.Generic.List[string]
foreach ($k in $cases.Keys) {
    $s = $cases[$k]
    $i = Run { $J::Parse($s).ToString([Newtonsoft.Json.Formatting]::Indented) }
    $c = Run { $J::Parse($s).ToString([Newtonsoft.Json.Formatting]::None) }
    $d = Run { [Newtonsoft.Json.JsonConvert]::DeserializeObject($s, $J).ToString([Newtonsoft.Json.Formatting]::None) }
    $out.Add(($k, (B64 $s), (B64 $i), (B64 $c), (B64 $d)) -join "`t")
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'json_cases.expected'), (($out -join "`n") + "`n"), $utf8)

# ToObject<T> for primitives: 18 values x 9 target types
$inv = [Globalization.CultureInfo]::InvariantCulture
$conv = New-Object System.Collections.Generic.List[string]
$types = @([int], [long], [double], [single], [bool], [string], [byte], [UInt32], [object])
$vals = @('5', '-5', '3000000000', '1.5', '2.5', '-1.5', '1e20', '"7"', '"x"', '"1.5"', 'true', 'false', 'null', '[1]', '{}', '"true"', '"2020-01-01T00:00:00Z"', '12345678901234567890')
$toObject = [Newtonsoft.Json.Linq.JToken].GetMethod('ToObject', [Type[]]@([Type]))
foreach ($v in $vals) { foreach ($t in $types) {
    try {
        $r = $toObject.Invoke($J::Parse($v), @($t))
        if ($r -eq $null) { $res = 'OK:null' }
        elseif ($r -is [double] -or $r -is [single]) { $res = 'OK:' + $r.GetType().Name + ' ' + $r.ToString('R', $inv) }
        elseif ($r -is [DateTime]) { $res = 'OK:DateTime ' + $r.Ticks + ' ' + $r.Kind }
        elseif ($r -is [Newtonsoft.Json.Linq.JToken]) { $res = 'OK:' + $r.GetType().Name + ' ' + $r.ToString('None') }
        else { $res = 'OK:' + $r.GetType().Name + ' ' + [string]$r }
    } catch { $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }; $res = 'ERR:' + $e.GetType().Name + ': ' + $e.Message }
    $conv.Add(($v, $t.Name, $res) -join "`t")
} }
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'json_convert.expected'), (($conv -join "`n") + "`n"), $utf8)

# BSON: the writer's bytes (hex) and what BsonReader reads back, for a few documents
$bson = New-Object System.Collections.Generic.List[string]
$bsonCases = [ordered]@{
    'empty-object' = '{}'
    'scalars' = ('{"i":5,"big":3000000000,"neg":-1,"f":1.5,"s":"h' + [char]0xE9 + 'llo","b":true,"n":null,"z":0}')
    'nested' = '{"o":{"a":[1,2,{"x":"y"}],"e":[]},"t":"2020-01-01T10:11:12Z"}'
    'unicode' = ('{"' + [char]0x65E5 + [char]0x672C + '":"' + [char]0x8A9E + [char]0x2028 + '"}')
    'achievements' = '{"ACH_A":{"Conditions":{"c1":{"Completed":true,"Value":3}}},"ACH_B":{"Conditions":{}}}'
}
foreach ($k in $bsonCases.Keys) {
    $tk = $J::Parse($bsonCases[$k])
    $ms = New-Object System.IO.MemoryStream
    $w = New-Object Newtonsoft.Json.Bson.BsonWriter($ms)
    $tk.WriteTo($w); $w.Flush()
    $bytes = $ms.ToArray()
    $ms2 = New-Object System.IO.MemoryStream(,$bytes)
    $r = New-Object Newtonsoft.Json.Bson.BsonReader($ms2)
    $back = $J::ReadFrom($r)
    $bson.Add(($k, (B64 $bsonCases[$k]), ([BitConverter]::ToString($bytes).Replace('-', '').ToLower()), (B64 ($back.ToString([Newtonsoft.Json.Formatting]::None)))) -join "`t")
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'bson_cases.expected'), (($bson -join "`n") + "`n"), $utf8)

# corpus: every embedded JSON resource, parsed with the (tolerant) JsonConvert path; hashes of the outputs
$corpus = New-Object System.Collections.Generic.List[string]
foreach ($f in Get-ChildItem (Join-Path $root 'decompile\Terraria') -Filter *.json | Sort-Object Name) {
    $text = [IO.File]::ReadAllText($f.FullName)
    $t = [Newtonsoft.Json.JsonConvert]::DeserializeObject($text, $J)
    $corpus.Add(($f.Name, $text.Length, (Sha ($t.ToString([Newtonsoft.Json.Formatting]::Indented))), (Sha ($t.ToString([Newtonsoft.Json.Formatting]::None)))) -join "`t")
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'json_corpus.expected'), (($corpus -join "`n") + "`n"), $utf8)
"cases: $($cases.Count); corpus files: $($corpus.Count)"
