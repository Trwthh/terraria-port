# Expected order for gfx::batcher::tests::sort_is_not_stable_and_matches_dotnet.
# Run with Windows PowerShell 5.1 (CLR 4). XNA's SpriteBatch.SortSprites calls Array.Sort(int[], 0, n, IComparer<int>).
# Terraria targets .NET Framework 4.0, so the CLR uses ArraySortHelper.DepthLimitedQuickSort for it; a process that
# targets 4.5+ (PowerShell) gets IntroSort instead. The script therefore prints BOTH: what this process's Array.Sort
# gives, and what the 4.0 algorithm gives (called through reflection when present). The .expected file holds the
# 4.0 line.
Add-Type -TypeDefinition @'
using System; using System.Collections.Generic; using System.Reflection;
public class BackToFront : IComparer<int> {
    public float[] d;
    public int Compare(int x, int y) { if (d[x] > d[y]) return -1; if (d[x] < d[y]) return 1; return 0; }
}
public static class SortProbe {
    public static string Legacy(int[] keys, IComparer<int> c) {
        Type t = typeof(object).Assembly.GetType("System.Collections.Generic.ArraySortHelper`1").MakeGenericType(typeof(int));
        MethodInfo m = t.GetMethod("DepthLimitedQuickSort", BindingFlags.NonPublic | BindingFlags.Static | BindingFlags.Public);
        if (m == null) return "no DepthLimitedQuickSort in this CLR";
        m.Invoke(null, new object[] { keys, 0, keys.Length - 1, c, 32 });
        return "[" + string.Join(", ", keys) + "]";
    }
}
'@
$d = [single[]](0.5,0.5,0.2,0.5,0.9,0.5,0.2,0.5,0.5,0.9,0.5,0.5,0.2,0.9,0.5,0.5,0.5,0.2,0.5,0.9)
$c = New-Object BackToFront; $c.d = $d
$idx = [int[]](0..19)
[Array]::Sort($idx, 0, 20, [System.Collections.Generic.IComparer[int]]$c)
"process: [" + ($idx -join ", ") + "]"
"net40:   " + [SortProbe]::Legacy([int[]](0..19), $c)
