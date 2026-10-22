# Expected values for the Regex tests of terraria-host: real .NET Framework (Windows PowerShell 5.1).
# Usage (from this folder): powershell -NoProfile -ExecutionPolicy Bypass -File regex_probe.ps1
# Writes regex_cases.expected: one line per (pattern, input): name TAB base64(input) TAB base64(matches) TAB base64(replace) TAB base64(split)
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
function B64($s) { [Convert]::ToBase64String($utf8.GetBytes($s)) }
function Esc($s) { $s.Replace('\', '\\').Replace("`r", '\r').Replace("`n", '\n').Replace("`t", '\t') }
$BS = [string][char]92
function P($pat, $o) { , @($pat, $o) }
$patterns = [ordered]@{
    # the game's patterns (decompile grep) with the options the game passes
    'seed-az09'      = (P ('[^a-z0-9]+') 0)
    'seed-AZaz09sp'  = (P ('[^a-zA-Z0-9 ]+') 0)
    'quote1'         = (P ('(' + $BS + $BS + '*)"') 0)
    'quote2'         = (P ('(' + $BS + $BS + '+)$') 0)
    'zip-dots'       = (P ('^[./]+') 0)
    'subst-utils'    = (P ('{(' + $BS + '?(?:!)?)?([a-zA-Z][' + $BS + 'w' + $BS + '.]*)}') 8)
    'subst-text'     = (P ('{(' + $BS + '?!?)?([a-zA-Z][' + $BS + 'w' + $BS + '.]*)}') 8)
    'locref'         = (P ('{' + $BS + '$(' + $BS + 'w+' + $BS + '.' + $BS + 'w+)}') 8)
    'hex'            = (P ('^[0-9a-f]+$') 0)
    'lines'          = (P ($BS + 'r' + $BS + 'n|' + $BS + 'r|' + $BS + 'n') 0)
    'filename'       = (P ('^(?<path>.*[' + $BS + $BS + $BS + '/])?(?:$|(?<fileName>.+?)(?:(?<extension>' + $BS + '.[^.]*$)|$))') 9)
    'chat-format'    = (P ('(?<!' + $BS + $BS + ')' + $BS + '[(?<tag>[a-zA-Z]{1,10})(' + $BS + '/(?<options>[^:]+))?:(?<text>.+?)(?<!' + $BS + $BS + ')' + $BS + ']') 24)
    # engine regression patterns (not from the game)
    'alt-backtrack'  = (P ('(a|ab)(c|bcd)(d*)') 0)
    'word-boundary'  = (P ($BS + 'bfoo' + $BS + 'b') 0)
    'inline-ci'      = (P ('(?i)hello') 0)
    'backref'        = (P ('(' + $BS + 'w+)' + $BS + 's' + $BS + '1') 0)
    'lazy-braces'    = (P ('a{2,3}?') 0)
    'lookahead'      = (P ('a(?=b)') 0)
    'neg-lookahead'  = (P ('a(?!b)') 0)
    'empty-star'     = (P ('x*') 0)
    'two-alt-groups' = (P ('(a)|(b)') 0)
    'multiline-blank' = (P ('^' + $BS + 's*$') 2)
    'mixed-numbering' = (P ('(?<n>a)(b)(?<m>c)(d)') 0)
    'lookbehind-pos' = (P ('(?<=a)b') 0)
    'nested-quant'   = (P ('(a+)+b') 0)
    'class-escapes'  = (P ('[' + $BS + 'w' + $BS + '-]+@[' + $BS + 'w.]+') 0)
    'singleline-dot' = (P ('a.c') 16)
    'ci-class'       = (P ('[a-c]+') 1)
    'anchors-end'    = (P ('b$') 0)
    'text-end-Z'     = (P ('b' + $BS + 'Z') 0)
}
$inputs = @(
    '', 'a', 'abc', 'Hello, World_42!', 'Not The Bees! 05162020', 'C:' + $BS + 'dir' + $BS + '"q"' + $BS, 'C:' + $BS + 'dir' + $BS + $BS, './../a/b', 'a/b/c.txt', '.hidden',
    'Hi {Name}, {?Flag}yes{?!Flag}no {x.y_1} {1bad} {}', 'x{$Mods.Name}y {$Bad} {$a.b}{$c.d}', '0a1f', '0g', "abc`n", "a`r`nb`rc`nd",
    'C:' + $BS + 'saves' + $BS + 'world.one.wld', 'C:' + $BS + 'saves' + $BS, 'world', 'a.b.c', 'dir/file.', '/rooted/name.ext',
    'hello [i:123] world', '[c/FF0000:red text] and [i/s5:29]', '[i:1][i:2]', 'esc ' + $BS + '[i:5]' + $BS + ' no tag', 'a ' + $BS + $BS + '[i:7] b', '[c/ff00ff:multi' + "`n" + 'line] x', '[g:name text]', '[toolongtagname:x]', '[a:]', '[a:b', 'plain text', '[i:5' + $BS + ']] after',
    'aab abb ab abcd abcdd', 'foo foobar barfoo foo.', 'HeLLo hello HELLO', 'the the cat cat sat', 'aaaa aa a', 'ab ac aba abb', 'abcd abcd', "line1`n   `nline3`n`n", 'abcdabcd', 'xxaxx', 'user-name@example.com, a@b.c', "a`nc abc", 'ABC abc AbC', 'ab', "ab`n", "a`nb", 'aaaaaaaaaaaaaaaaaaaaaaaac', 'caf' + [char]0xE9 + ' ' + [char]0x65E5 + [char]0x672C + ' ' + [char]0xD83D + [char]0xDE00 + ' [i:9]'
)
$sep = [string][char]0xA6
$lines = New-Object System.Collections.Generic.List[string]
foreach ($name in $patterns.Keys) {
    $pat = $patterns[$name][0]; $opt = [Text.RegularExpressions.RegexOptions]$patterns[$name][1]
    $re = New-Object Text.RegularExpressions.Regex($pat, $opt)
    foreach ($in in $inputs) {
        $sb = New-Object System.Text.StringBuilder
        foreach ($m in $re.Matches($in)) {
            [void]$sb.Append("M $($m.Index) $($m.Length) [$(Esc $m.Value)]`n")
            for ($i = 1; $i -lt $m.Groups.Count; $i++) {
                $g = $m.Groups[$i]
                [void]$sb.Append("  G$i $($re.GroupNameFromNumber($i)) $($g.Success) $($g.Index) $($g.Length) [$(Esc $g.Value)]`n")
            }
        }
        $first = $re.Match($in)
        [void]$sb.Append("first $($first.Success) isMatch $($re.IsMatch($in))`n")
        $rep = $re.Replace($in, '<$0|$1|${0}>')
        $split = ($re.Split($in) | ForEach-Object { Esc $_ }) -join $sep
        $lines.Add(($name, (B64 $in), (B64 $sb.ToString()), (B64 (Esc $rep)), (B64 $split)) -join "`t")
    }
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'regex_cases.expected'), (($lines -join "`n") + "`n"), $utf8)
$pl = ($patterns.Keys | ForEach-Object { $_ + "`t" + (B64 $patterns[$_][0]) + "`t" + $patterns[$_][1] }) -join "`n"
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'regex_patterns.expected'), ($pl + "`n"), $utf8)
"patterns: $($patterns.Count); inputs: $($inputs.Count); cases: $($lines.Count)"
