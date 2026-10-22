# Expected values for the codec tests of terraria-host: the game's own NVorbis.dll and MP3Sharp.dll (meta\refs).
# Fixtures: ..\data\tone_*.ogg / tone_*.mp3 (made with ffmpeg: two sines at 44.1 kHz stereo, one at 22.05 kHz mono).
# Usage (from this folder): powershell -NoProfile -ExecutionPolicy Bypass -File codec_probe.ps1
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..\..')
$data = Join-Path $PSScriptRoot '..\data'
Add-Type -Path (Join-Path $root 'meta\refs\NVorbis.dll')
Add-Type -Path (Join-Path $root 'meta\refs\MP3Sharp.dll')
$inv = [Globalization.CultureInfo]::InvariantCulture
function F($v) { ([double]$v).ToString('R', $inv) }
$out = New-Object System.Collections.Generic.List[string]
foreach ($f in 'tone_stereo.ogg', 'tone_mono.ogg') {
    $s = [IO.File]::OpenRead((Join-Path $data $f)); $r = New-Object NVorbis.VorbisReader($s)
    $tags = ($r.Tags.All.Keys | Where-Object { $_ -ne 'ENCODER' } | Sort-Object | ForEach-Object { $_ + '=' + ($r.Tags.All[$_] -join ',') }) -join ';'
    $buf = New-Object 'single[]' 2048; $all = New-Object System.Collections.Generic.List[single]
    while (($n = $r.ReadSamples($buf, 0, $buf.Length)) -gt 0) { for ($i = 0; $i -lt $n; $i++) { $all.Add($buf[$i]) } }
    $sum = 0.0; $peak = 0.0; foreach ($v in $all) { $sum += $v * $v; if ([math]::Abs($v) -gt $peak) { $peak = [math]::Abs($v) } }
    $out.Add("$f`tchannels`t$($r.Channels)"); $out.Add("$f`trate`t$($r.SampleRate)"); $out.Add("$f`ttotal`t$($r.TotalSamples)")
    $out.Add("$f`ttags`t$tags"); $out.Add("$f`tfloats`t$($all.Count)"); $out.Add("$f`tposition_at_end`t$($r.DecodedPosition)")
    $out.Add("$f`trms`t$(F ([math]::Sqrt($sum / $all.Count)))"); $out.Add("$f`tpeak`t$(F $peak)")
    foreach ($i in 0, 1, 1000, 20001, 20002, 40000) { $out.Add("$f`tsample $i`t$(F $all[$i])") }
    # seek: DecodedPosition = 5000, read 8 floats
    $r.DecodedPosition = 5000; $n = $r.ReadSamples($buf, 0, 8)
    $out.Add("$f`tseek5000`t$n $($r.DecodedPosition) $(F $buf[0]) $(F $buf[1]) $(F $buf[7])")
    # the game's loop: read to the end, DecodedPosition = loopStart
    $r.DecodedPosition = 10000; $n = $r.ReadSamples($buf, 0, 4)
    $out.Add("$f`tseek10000`t$n $($r.DecodedPosition) $(F $buf[0]) $(F $buf[3])")
    $r.DecodedPosition = 0; $n = $r.ReadSamples($buf, 0, 4)
    $out.Add("$f`tseek0`t$n $($r.DecodedPosition) $(F $buf[0]) $(F $buf[3])")
    $r.Dispose()
}
foreach ($f in 'tone_stereo.mp3', 'tone_mono.mp3') {
    $s = [IO.File]::OpenRead((Join-Path $data $f)); $m = New-Object XPT.Core.Audio.MP3Sharp.MP3Stream($s)
    $buf = New-Object 'byte[]' 4096; $bytes = New-Object System.Collections.Generic.List[byte]
    while (($n = $m.Read($buf, 0, $buf.Length)) -gt 0) { for ($i = 0; $i -lt $n; $i++) { $bytes.Add($buf[$i]) } }
    $arr = $bytes.ToArray(); $total = $arr.Length
    $sum = 0.0; $peak = 0; for ($i = 0; $i -lt $total; $i += 2) { $v = [BitConverter]::ToInt16($arr, $i); $sum += [double]$v * $v; if ([math]::Abs($v) -gt $peak) { $peak = [math]::Abs($v) } }
    $out.Add("$f`tfrequency`t$($m.Frequency)"); $out.Add("$f`tbytes`t$total"); $out.Add("$f`tlength`t$($m.Length)"); $out.Add("$f`tposition_at_end`t$($m.Position)")
    $out.Add("$f`trms`t$(F ([math]::Sqrt($sum / ($total / 2))))"); $out.Add("$f`tpeak`t$peak")
    foreach ($i in 4000, 20000, 40001, 45000) { $out.Add("$f`tsample $i`t$([BitConverter]::ToInt16($arr, $i * 2))") }
    $m.Position = 0; $n = $m.Read($buf, 0, 16)
    $out.Add("$f`trewind`t$n $([BitConverter]::ToInt16($buf, 0))")
    $m.Close()
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'codec.expected'), (($out -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding($false)))
"lines: $($out.Count)"
