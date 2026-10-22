# Expected values for the System.* unit tests of terraria-host, produced by real .NET Framework (Windows PowerShell 5.1, CLR 4).
# Usage: powershell -NoProfile -File system_probe.ps1 > system_probe.out
$ErrorActionPreference = 'Stop'
function P($name, $value) { "{0} = {1}" -f $name, $value }
function Hex($bytes) { ($bytes | ForEach-Object { $_.ToString('x2') }) -join '' }

"## Path"
foreach ($p in 'C:\a\b\file.txt', 'C:\a\b\', 'file', 'a.b.c', '.hidden', 'C:\', 'a/b\c.d', '', 'dir.x\file', 'C:\a\b') {
    P "GetFileName($p)" ("[" + [IO.Path]::GetFileName($p) + "]")
    P "GetFileNameWithoutExtension($p)" ("[" + [IO.Path]::GetFileNameWithoutExtension($p) + "]")
    P "GetExtension($p)" ("[" + [IO.Path]::GetExtension($p) + "]")
    if ($p -ne '') { P "GetDirectoryName($p)" ("[" + [IO.Path]::GetDirectoryName($p) + "]") }
    P "HasExtension($p)" ([IO.Path]::HasExtension($p))
    P "IsPathRooted($p)" ([IO.Path]::IsPathRooted($p))
}
P "ChangeExtension(a\b.txt,.bak)" ([IO.Path]::ChangeExtension('a\b.txt', '.bak'))
P "ChangeExtension(a\b,bak)" ([IO.Path]::ChangeExtension('a\b', 'bak'))
P "ChangeExtension(a\b.txt,null)" ([IO.Path]::ChangeExtension('a\b.txt', $null))
P "InvalidFileNameChars" (([IO.Path]::GetInvalidFileNameChars() | ForEach-Object { [int]$_ }) -join ',')
P "InvalidPathChars" (([IO.Path]::GetInvalidPathChars() | ForEach-Object { [int]$_ }) -join ',')
P "DirectorySeparatorChar" ([int][IO.Path]::DirectorySeparatorChar)

"## DateTime"
$d = New-Object DateTime 2024, 2, 29, 13, 5, 9, 123, ([DateTimeKind]::Utc)
P "Ticks" $d.Ticks
P "ToBinary(utc)" $d.ToBinary()
$u = New-Object DateTime 2024, 2, 29, 13, 5, 9, 123, ([DateTimeKind]::Unspecified)
P "ToBinary(unspecified)" $u.ToBinary()
P "FromBinary(utc).Ticks" ([DateTime]::FromBinary($d.ToBinary()).Ticks)
P "FromBinary(utc).Kind" ([DateTime]::FromBinary($d.ToBinary()).Kind)
P "parts" ("{0} {1} {2} {3} {4} {5} {6} {7} {8}" -f $d.Year, $d.Month, $d.Day, $d.Hour, $d.Minute, $d.Second, $d.Millisecond, [int]$d.DayOfWeek, $d.DayOfYear)
P "AddSeconds(86400.5).Ticks" $d.AddSeconds(86400.5).Ticks
P "AddSeconds(-0.0004).Ticks" $d.AddSeconds(-0.0004).Ticks
P "AddDays(366).Ticks" $d.AddDays(366).Ticks
P "Subtract" ($d - (New-Object DateTime 1999, 12, 31, 23, 59, 59)).Ticks
$inv = [Globalization.CultureInfo]::InvariantCulture
$en = [Globalization.CultureInfo]::GetCultureInfo('en-US')
P "ToString(D,en-US)" $d.ToString('D', $en)
P "ToString(MMMM d, yyy,en-US)" $d.ToString('MMMM d, yyy', $en)
P "ToString(yyyyMMddHHmmssfff)" $d.ToString('yyyyMMddHHmmssfff', $inv)
P "MinValue.Ticks" ([DateTime]::MinValue.Ticks)
P "MaxValue.Ticks" ([DateTime]::MaxValue.Ticks)
P "Epoch1970.Ticks" (New-Object DateTime 1970, 1, 1).Ticks
P "FileTimeEpoch.Ticks" (New-Object DateTime 1601, 1, 1).Ticks
$dates = @((New-Object DateTime 1, 1, 1), (New-Object DateTime 1900, 3, 1), (New-Object DateTime 2000, 12, 31), (New-Object DateTime 2100, 3, 1), (New-Object DateTime 9999, 12, 31, 23, 59, 59))
foreach ($x in $dates) { P ("date " + $x.Ticks) ("{0} {1} {2} {3}" -f $x.Year, $x.Month, $x.Day, [int]$x.DayOfWeek) }

"## TimeSpan / Stopwatch"
P "Stopwatch.Frequency>0" ([Diagnostics.Stopwatch]::Frequency -gt 0)
P "Stopwatch.IsHighResolution" ([Diagnostics.Stopwatch]::IsHighResolution)

"## Random"
$r = New-Object Random 12345
P "Random(12345).Next x5" ((1..5 | ForEach-Object { $r.Next() }) -join ',')
P "Random(12345).NextDouble" $r.NextDouble().ToString('R', $inv)
P "Random(12345).Next(10)" $r.Next(10)
P "Random(12345).Next(5,50)" $r.Next(5, 50)
$r0 = New-Object Random 0
P "Random(0).NextDouble x3" ((1..3 | ForEach-Object { $r0.NextDouble().ToString('R', $inv) }) -join ',')
$rn = New-Object Random (-7)
P "Random(-7).Next x3" ((1..3 | ForEach-Object { $rn.Next() }) -join ',')

"## Convert / Uri / Encoding"
P "ToBase64String" ([Convert]::ToBase64String([byte[]](0, 1, 2, 250, 251, 252, 253, 254, 255, 7)))
P "ToBase64String(empty)" ("[" + [Convert]::ToBase64String([byte[]]@()) + "]")
P "EscapeDataString" ([Uri]::EscapeDataString("a b&c=d/e?f#g+h~i_j-k.l!m*n'o(p)" + [char]0xE9 + [char]0x4E2D))
P "UnescapeDataString" ([Uri]::UnescapeDataString('a%20b%26c%C3%A9%4e%zz+%'))
P "Unicode.GetBytes(h3y_gUyZ)" (Hex ([Text.Encoding]::Unicode.GetBytes('h3y_gUyZ')))
P "UTF8.GetByteCount" ([Text.Encoding]::UTF8.GetByteCount("a" + [char]0xE9 + [char]0x4E2D))
P "ASCII.GetString" ([Text.Encoding]::ASCII.GetString([byte[]](72, 105, 200, 33)))
P "ASCII.GetBytes" (Hex ([Text.Encoding]::ASCII.GetBytes("Hi" + [char]0xE9 + "!")))

"## Number formatting"
P "byte X2" ((0, 9, 10, 171, 255 | ForEach-Object { ([byte]$_).ToString('X2') }) -join ',')
P "float F2" ((0.005, 1.005, 2.675, -0.004, 1234.5, 0.125, 0.135 | ForEach-Object { ([single]$_).ToString('F2', $inv) }) -join ',')
P "float P0" ((0.5, 1.25, 0.004, 0.995, -0.2 | ForEach-Object { ([single]$_).ToString('P0', $inv) }) -join '|')
P "Math.Round(x,2)" ((2.675, 2.665, 1.005, -2.5, 0.125 | ForEach-Object { [Math]::Round([double]$_, 2).ToString('R', $inv) }) -join ',')
P "Math.Round(x,0)" ((0.5, 1.5, 2.5, -0.5 | ForEach-Object { [Math]::Round([double]$_, 0).ToString('R', $inv) }) -join ',')
P "Math.Log(8,2)" ([Math]::Log(8, 2).ToString('R', $inv))
P "Math.Log(100,10)" ([Math]::Log(100, 10).ToString('R', $inv))

"## AES (RijndaelManaged defaults: CBC, PKCS7, block 128), key = IV = Unicode('h3y_gUyZ')"
$key = [Text.Encoding]::Unicode.GetBytes('h3y_gUyZ')
foreach ($len in 0, 1, 15, 16, 17, 40) {
    $plain = [byte[]](0..($len - 1) | Where-Object { $len -gt 0 } | ForEach-Object { ($_ * 7 + 3) % 256 })
    if ($len -eq 0) { $plain = [byte[]]@() }
    $rm = New-Object Security.Cryptography.RijndaelManaged
    $enc = $rm.CreateEncryptor($key, $key)
    $ms = New-Object IO.MemoryStream
    $cs = New-Object Security.Cryptography.CryptoStream $ms, $enc, ([Security.Cryptography.CryptoStreamMode]::Write)
    $cs.Write($plain, 0, $plain.Length); $cs.FlushFinalBlock()
    P "encrypt($len)" (Hex $ms.ToArray())
}
$rm = New-Object Security.Cryptography.RijndaelManaged
P "defaults" ("{0} {1} {2} {3}" -f $rm.Mode, $rm.Padding, $rm.BlockSize, $rm.KeySize)

"## Regex"
P "seed" ([regex]::Replace("Hello, World_42! " + [char]0xE9, '[^a-z0-9]+', ''))
P "seed2" ([regex]::Replace('Not The Bees! 05162020', '[^a-zA-Z0-9 ]+', ''))
P "quote1" ([regex]::Replace('C:\dir\"q"\', '(\\*)"', '$1$1\"'))
P "quote2" ([regex]::Replace('C:\dir\\', '(\\+)$', '$1$1'))
P "zip" ([regex]::Replace('./../a/b', '^[./]+', ''))
$subst = New-Object regex '{(\?(?:!)?)?([a-zA-Z][\w\.]*)}'
P "subst" ($subst.Replace('Hi {Name}, {?Flag}yes{?!Flag}no {x.y_1} {1bad} {}', { param($m) "<" + $m.Groups[1].Value + "|" + $m.Groups[2].Value + "|" + $m.Groups[1].Length + ">" }))

"## BinaryWriter (cross-check of the runtime's rt::io)"
$ms = New-Object IO.MemoryStream
$bw = New-Object IO.BinaryWriter $ms
$bw.Write([string]("a" + [char]0xE9 + [char]0x4E2D)); $bw.Write([string]('x' * 200)); $bw.Write([single]1.5); $bw.Write([int64]-2); $bw.Write($true); $bw.Write([uint16]65535)
$bw.Flush()
P "BinaryWriter" (Hex $ms.ToArray())
