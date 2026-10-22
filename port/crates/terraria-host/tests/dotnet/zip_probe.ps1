# Expected values for the zip tests of terraria-host: a zip made with System.IO.Compression and read back with the
# game's own Ionic.Zip.CF.dll (meta\refs); also deflate streams made by Ionic.Zlib.
# Usage (from this folder): powershell -NoProfile -ExecutionPolicy Bypass -File zip_probe.ps1
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..\..')
Add-Type -Path (Join-Path $root 'meta\refs\Ionic.Zip.CF.dll')
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$data = Join-Path $PSScriptRoot '..\data'
$utf8 = New-Object System.Text.UTF8Encoding($false)
function Sha($bytes) { $h = [System.Security.Cryptography.SHA256]::Create(); ([BitConverter]::ToString($h.ComputeHash($bytes))).Replace('-', '').ToLower() }
$zipPath = Join-Path $data 'pack.zip'
if (Test-Path $zipPath) { Remove-Item $zipPath }
$rnd = New-Object System.Random(42)
$bin = New-Object byte[] 3000; $rnd.NextBytes($bin)
for ($i = 0; $i -lt 1500; $i++) { $bin[$i] = [byte]($i % 7) }    # half compressible
$text = $utf8.GetBytes((('Resource pack line {0}' + "`r`n") * 1) -f 1) 
$longText = $utf8.GetBytes((1..400 | ForEach-Object { "Line $_ of the pack description, repeated text compresses well." }) -join "`n")
$fs = [IO.File]::Open($zipPath, 'Create')
$zip = New-Object System.IO.Compression.ZipArchive($fs, [System.IO.Compression.ZipArchiveMode]::Create)
function Add($name, $bytes, $level) {
    $e = $zip.CreateEntry($name, $level)
    $s = $e.Open(); $s.Write($bytes, 0, $bytes.Length); $s.Close()
}
Add 'pack.json' ($utf8.GetBytes('{"name":"Test pack","description":"x","author":"me","version":{"major":1,"minor":0}}')) 'Optimal'
Add 'Content/Images/Item_1.png' $bin 'Optimal'
Add 'Content/Images/Sub/Tile_0.png' $bin 'Fastest'
Add 'Content/Description.txt' $longText 'Optimal'
Add 'Content/stored.bin' $bin 'NoCompression'
Add 'Content/empty.txt' (New-Object byte[] 0) 'Optimal'
Add ('Content/caf' + [char]0xE9 + '.txt') $text 'Optimal'
[void]$zip.CreateEntry('Content/Folder/')
$zip.Dispose(); $fs.Close()

$z = [Ionic.Zip.ZipFile]::Read($zipPath)
$lines = New-Object System.Collections.Generic.List[string]
foreach ($e in $z.Entries) {
    $ms = New-Object IO.MemoryStream
    if (-not $e.IsDirectory) { $e.Extract($ms) }
    $lines.Add(($e.FileName, $e.IsDirectory, $e.UncompressedSize, $e.CompressedSize, $e.Crc, (Sha $ms.ToArray())) -join "`t")
}
$lines.Add('lookup' + "`t" + $z.ContainsEntry('content\images\item_1.png') + "`t" + $z.ContainsEntry('Content/Images/Item_1.png') + "`t" + $z.ContainsEntry('nope') + "`t" + ($z['CONTENT/DESCRIPTION.TXT'] -ne $null) + "`t" + $z.Entries.Count)
$z.Dispose()
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'zip.expected'), (($lines -join "`n") + "`n"), $utf8)

# deflate fixtures: raw deflate made by Ionic.Zlib.DeflateStream and a zlib stream made by Ionic.Zlib.ZlibStream
$payload = New-Object byte[] 20000
for ($i = 0; $i -lt $payload.Length; $i++) { $payload[$i] = [byte](($i * 7 + ($i / 13)) % 251) }
$ms = New-Object IO.MemoryStream
$d = New-Object Ionic.Zlib.DeflateStream($ms, [Ionic.Zlib.CompressionMode]::Compress, $true); $d.Write($payload, 0, $payload.Length); $d.Dispose()
$raw = $ms.ToArray()
$ms = New-Object IO.MemoryStream
$zs = New-Object Ionic.Zlib.ZlibStream($ms, [Ionic.Zlib.CompressionMode]::Compress, $true); $zs.Write($payload, 0, $payload.Length); $zs.Dispose()
$zl = $ms.ToArray()
$def = @("payload`t$(Sha $payload)`t$($payload.Length)", "raw`t$([Convert]::ToBase64String($raw))", "zlib`t$([Convert]::ToBase64String($zl))")
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'deflate.expected'), (($def -join "`n") + "`n"), $utf8)
"entries: $($lines.Count - 1); raw $($raw.Length) bytes, zlib $($zl.Length) bytes"
