# When the original keeps a `float` at more than float precision

Target: 32-bit .NET Framework 4 (CLR 4.0.30319.42000, legacy x86 JIT "JIT32"), x87 control word `0x027F` (53-bit precision). Everything
below is established by probes run in that runtime; each statement names the probe(s). Probe sets: `P..` = first set
(`refdata/worldgen/float_probes`, source `probes_source.cs`), all others = second set (`refdata/worldgen/float_probes2`, source
`probes2_source.cs`, machine code of every probe in `probes2_code.bin`). 2,000 inputs per probe, results compared bit for bit.
Reference model and scores: `tools/refdump_worldgen/float_model` (`python score.py`, `python hand.py`, `python calls.py`,
`python disasm.py NAME`). Where a statement comes from the published source of the same JIT family (dotnet/coreclr release/2.0.0,
`src/jit/stackfp.cpp`, `regalloc.cpp`, `compiler.h`; MIT licence) and no probe separates it, it is marked **[source]**.

## 1. The rule in short

1. Arithmetic on `float` operands is done at double precision and is not rounded per operation. A value is rounded to `float` only
   when it is written to memory as a float or cast with `(float)`.
2. A `float` local or parameter is either **in memory** (every store to it rounds) or **on the x87 stack** (stores do not round).
   Which one is decided per method by the JIT's register allocator (section 3). At most six float variables can be on the stack at the
   same time.
3. A variable on the x87 stack is rounded to float at every call to a non-inlined method that runs while the variable holds a value
   that is read later (the JIT spills it to a 4-byte slot around the call).
4. Float arguments are rounded at the call, inlined or not (inlined: probed for expression arguments). A float return value is NOT
   rounded: the caller continues with the unrounded value of the return expression of the callee.
5. Methods compiled without optimisation because of their size ("MinOpts") keep every local in memory: there, the rule "every store
   rounds" is exact.

## 2. What rounds and what does not (independent of register allocation)

| construct | rounds to float? | evidence |
|---|---|---|
| `+ - * /` on floats, nested | no (53-bit results) | P02, P22, P23, P28 |
| comparison of float expressions | operands unrounded | P09, P08, S05 |
| `(int)expr` | truncates the unrounded value | P06, P07, P24, M05 |
| `(float)expr` | yes, also when `expr` is an unrounded local | M03, M03b, P19; M03c (`(float)a - x` differs from P01's `a - x` on 521 of the same 2000 inputs) |
| `(double)expr` on a float expression | no | P17, P18, I06 |
| `(float)intExpr` (also the implicit conversion in `f + i`) | yes (`fild; fstp dword; fld dword` in the code of I08) | machine code of I08 only; P29 does not separate it |
| store to a static field, instance field, array element | yes | P04, P05, M01, V04 |
| store to a local / parameter | only when the variable is in memory (section 3) | P01, P03, P10, P11, P31, S01, S03, S03b, S04, S06 to S08 |
| local passed by `ref` | in memory: yes | P34, M06 (the other local of M06 stays unrounded) |
| float argument of a non-inlined call | yes (`fstp dword [esp]`) | P16, C07, C08, C10 |
| float argument of an INLINED call, argument is an expression | yes (stored to a float temporary and reloaded) | I02, I05 (`hand.py`: 2000 vs 1893 / 1421 for "unrounded"). Not probed: an unrounded local passed as the argument |
| double argument of an inlined call | no | I06 |
| float return value | no: the caller sees the unrounded return expression | C09, V02, V05, V08 (`hand.py`: 2000 vs 1479 / 1537 / 1352 / 1507 for "rounded") |
| expression temporary pending across a call (`x * y + F(z)`) | no (spilled to an 8-byte slot) | C04; machine code of I08 (`fstp qword`) |
| the value of an enregistered local already loaded when a call in the same expression runs (`a + F(z)`) | yes | C05 |
| `Math.Abs(float)`, `Math.Sqrt(floatExpr)` argument | no rounding of the argument (`fabs` / `fsqrt` on the stack value); `(float)Math.Sqrt` rounds by its cast | P19, P20, `x87_more/AbsMulF` |
| `Math.Max(float, float)`, `Math.Min` | arguments rounded | P27 |
| `Math.Sin / Cos / Tan / Atan / Atan2` result | keeps the 64-bit register significand until stored or cast | crate README section 1 (`x87_more/SinMul` etc.) |
| fields of a local struct (own struct, XNA `Vector2`) | behave as separate locals (the JIT promotes the struct) | M02, P12, P13, V07, I07 |
| a struct on which a non-inlined instance method is called (`v.Length()`, `v.Normalize()`, `v.LengthSquared()`) | the struct is in memory: its fields are rounded when written | V03, V08, P15 |
| `double` locals and fields | no difference to IEEE double arithmetic | `x87_more/DblLocal`, `DblField` |

## 3. Which float variables live on the x87 stack

Variables: the method's float parameters and float locals, plus fields of promoted local structs. Definitions used below: a
*reference* is one read or one write of the variable in the source (`a += e` is two); the *weight* of a reference is 1, times 8 for every
enclosing loop **[source]**; a variable is *live* at a point when its current value is read later on some path; two variables *interfere*
when one is written while the other is live (parameters are written at entry).

```
in_memory(method) -> set of float variables kept in memory
  if method is MinOpts (section 5):                         return all
  M = {}
  for v in float variables:
    if address of v is taken (ref / out / &, or v is a struct with a non-inlined instance call):   M += v     # P34, M06, V03, V08, P15
    if v is live on entry to a catch / finally handler:                                            M += v     # T03 (T01, T02: not live there -> stack)
    if v is not among the 64 locals of the method with the highest weighted reference count
       (all locals and parameters of all types count):                                             M += v     # B10 (46 locals: stack) vs B11, B12, B13 (76+: memory)
    if v is a parameter with at most one reference:                                                M += v     # [source]; has no numeric effect (never written)
    payload(v) = sum over calls c to non-inlined methods with v live across c of 2 * weight(c)
    if weighted_refs(v) - payload(v) < 1:                                                          M += v     # C02 (3 refs, 1 call: stack), C03 (4 refs, 2 calls: memory), C12 (6 refs, 2 calls: stack), P32
  candidates = float variables not in M, ordered by weighted_refs descending, then by local number (parameters first, then locals in
               declaration order)
  six registers; for each candidate in order: give it the first register none of whose current holders interferes with it;
                                              if there is none:                                    M += v     # P31, L06, L09, L10, L13
  return M
```

Then, when evaluating the method:

* store to `v`: round to float iff `v` is in memory;
* at every call to a non-inlined method: round to float every stack variable that is live across the call (C01, C02, C11, C12, P33;
  machine code of C01: `fstp dword [ebp-4]` before the call, `fld dword [ebp-4]` after it);
* everything else as in section 2.

What the probes establish about the allocation, one by one:

* locals with a single assignment and a few uses are on the stack: P01, P03, P08, P10, P11 (the "round at every store" rule misses
  521 to 1856 of 2000 there);
* six can be live together (L06); what counts is how many are live at the same time, not how many the method has (L09: eight locals,
  four at a time, all on the stack; L10: seven, the first dead before the seventh is written, all on the stack);
* float parameters that are read twice or more are candidates like locals and hold a register while they are live (machine code of
  L13 and C21: `fld dword [ebp+0x10]` in the prologue, the value then stays on the stack). L13 still fits seven variables into six
  registers because `x` is dead when the sixth local is written;
* with ten locals live together the six with the most references are on the stack (P31: `a0..a5` have 4 references, the others 3 or
  2). The earlier reading "the first six by declaration" was a coincidence of that probe;
* a local live across a call needs more than two references per call to stay on the stack, and is then rounded at the call (C01, C02,
  C03, C12);
* a local assigned in both branches of an `if`, a conditional expression, loop-carried locals, two interdependent accumulators,
  nested loops, a parameter modified in place: all on the stack, unrounded (S01, S04, P10, P11, S06, S07, S08, S03, S03b);
* `try` / `catch`: only variables live into the handler are forced to memory (T01, T02, T03).

## 4. Undetermined (the rule above is known to be incomplete here)

1. **More than six candidates interfere and several have the same weight.** The procedure of section 3 breaks ties by local number;
   the JIT does not. L07 (seven locals, `a4 a5 a6` tied for two registers): the JIT keeps `a4, a6`, the procedure `a4, a5`. L12 (the
   same with the declarations reversed): JIT `a4, a5`, procedure `a5, a6`. L08: JIT drops `a0` and `a5`, the procedure `a4` and `a5`.
   The sets were found by exhaustive search (`score.py --oracle`: exactly one set of stack variables reproduces each probe). No order
   by declaration, assignment or reference count explains all three; the published selection loop does not either. Effect: the rule
   knows THAT one variable of the group is rounded, not which one.
2. **Temporaries created by the JIT compete for the six registers.** A subexpression that occurs twice is computed once and kept in
   a register (C20, machine code: one `fmul`). In C21 three such temporaries plus three parameters fill the six registers and the
   locals assigned from them go to memory (JIT: only `a3 a4 a5` on the stack). The procedure does not model this (C21 missed).
   L14 (parameters with two references against locals with two) is the same class.
3. **Loop weights and the expression-depth term.** The factor 8 per loop level, the halving for blocks that are not executed on every
   iteration, and the penalty the JIT applies when live stack variables plus expression depth exceed eight slots are **[source]**;
   no probe isolates them (every loop probe is reproduced with or without the factor).
4. **MinOpts thresholds other than the instruction count** (section 5).
5. **Which calls are inlined** beyond the cases of section 6. This matters for rule 3 of section 1 (an inlined call does not spill)
   and for the payload term.
6. **Operand order.** In `a + F(z)` the already loaded value of `a` is rounded (C05). Whether the JIT evaluates the call first for
   other operand shapes was not probed.
7. Subnormal results and values beyond the float / double range held in registers (wider exponent range of the x87 register).
8. **The variables of the rule are the JIT's IL locals, not the decompiler's.** The C# compiler reuses an IL local slot for
   several source variables and the decompiler splits it again, so a decompiled local can show fewer references, and fewer calls
   across its lifetime, than the slot the JIT allocates. Measured in the game: `Main.UpdateWeather`, local `num7`
   (`num7 += Math.Abs(num8 - windSpeedCurrent) * 0.0015f`, three references, no call across it in the decompiled text) is IN
   MEMORY in the real server: `Main.windSpeedCurrent` of `refdata/ticks` (world scenario, tick 1,560) is reproduced only with
   `num7` rounded at its store. The translator lists such locals in its config (`tools/cs2rs/game.json`, `floatInMemory`) with
   the evidence; applying section 3 to the IL locals (they are in `meta/`) would remove the need for the list.

## 5. Methods compiled without optimisation

A method is compiled as "MinOpts" (no register allocation for locals at all) when it is too large. Established: B02 (2,000 filler
statements, 32,026 bytes of IL, about 16,000 IL instructions) is optimised; B03 (41,626 bytes, about 20,800 instructions), B03b
(56,026 bytes, 28,000 instructions) and B01 (64,026 bytes, 32,000 instructions) are not: every float local is rounded at every store
there. That matches the published limit of 20,000 IL instructions. The other published limits (IL larger than 60,000 bytes, more than
2,000 basic blocks, more than 2,000 locals, more than 8,000 local references) are **[source]**: B01 exceeds the size limit but also
the instruction limit. The game's largest methods are far beyond 20,000 instructions, so for them "round at every store to a local"
(plus section 2) is the exact rule.

## 6. Inlining (what the machine code of the probes shows)

| callee | inlined? | probe |
|---|---|---|
| `int f(int)` small | yes | I01 |
| small method with float parameters returning `int`, `bool` or `void` | yes; an argument that is an expression is rounded to float | I02, I04, I05 |
| small method with `double` parameter and result | yes, nothing rounded | I06 |
| small method taking and returning a struct of floats | yes, fields stay unrounded | I07 |
| property getter returning a float field (`return w;`) | yes | I08 |
| method or getter whose float return value is an arithmetic expression (`return a * 1.5f;`, `return w * 0.5f;`, `return a * b + c;`) | **no**, whatever its size | C07, C08, C10, I03, I08 (`Half`), V02, V05 |
| XNA `Vector2` constructor, operators `+`, `-`, `*` (vector, float), `+=`, `*=` | yes | P12, P13, V04, V07 |
| XNA `Vector2.Length`, `LengthSquared`, `Distance`, `Dot`, `MathHelper.Lerp`, `MathHelper.Clamp` | no (call instruction present) | V01, V02, V03, V05, V06, V08 |
| XNA `Vector2.Normalize` | no machine code in the first set; its values fit "called, struct in memory" | P15 |

Read from the game's own process (2026-10-14, the collision reference run, `work\jit\collref`; tools\cs2rs
VERIFICATION.md section D): the XNA members are compiled by the same JIT and keep THEIR intermediates unrounded:
`Vector2.Length()` = `(float)sqrt(X*X + Y*Y)` with the sum at double precision (`fstp qword` / `fld qword`, `fsqrt`,
`fstp dword`); `LengthSquared`, `DistanceSquared`, `Dot` return the unrounded sum; `Distance` as `Length`;
`Normalize` rounds the length to float, keeps `1 / length` unrounded and rounds each product at its store;
`op_Division(Vector2, float)` multiplies by an unrounded `1 / d`. A JIT allocation helper (`newarr`) is a call
instruction: x87 locals live across it are spilled as floats (`CheckLinevLine.num4`).

Read from the lighting reference run (2026-10-14, `work\jit\lightref`, refdata\light reproduced byte for byte):
- the spill at a call happens where the call is: `Tile.slope()` is a call in `ApplyHellLight` / `ApplySurfaceLight`
  (not inlined in that run), inside the right operand of `||`; the four x87 values (num4 or num7 and the zeros) are
  stored as floats only on the path that reaches it and stay unrounded on the others;
- an inlined callee still spills what is live when its body calls something (`FastRandom.Next(0, 50)` inlined,
  `call Next(int)` inside; `num4 += ...` spills num4 before it, C05);
- `Math.Sin` / `Math.Cos` are `fsin` / `fcos`: no call, nothing spilled; the argument goes through a `qword` slot;
- float constant expressions are folded at float precision (`0.7f - 0.2f` with both locals constant: `0.5f`);
- XNA `Vector4.Lerp` keeps `(v2 - v1) * t + v1` unrounded per component, one rounding at the store.

Read from every reference run (2026-10-14, later; `tools\refdump_worldgen\run_refjit.sh`, `work\jit\ref\<scenario>`):
p1 start-up, framing (plain and scrambled), collision, lighting, sky, the six liquid scenarios, both tick scenarios and
the six world generation runs are each repeated by their own harness code (sources compiled in unchanged), every
reference file reproduced byte for byte (only wall-clock fields differ, `refjit_compare.py`), and then the code of every
game / ReLogic / XNA method that ran in that process is read without compiling anything (`JitDump.cs DumpRan`; 1,593 to
2,840 methods per run, 4,256 in the union, 309 of them with a float or double local). What these 20 processes show:
- the CLR keeps a code header in front of each jitted method (`[entry-4]` = its MethodDesc, `[entry-8]` = its GC info,
  whose first value is the code size): which methods ran, and their exact code, are read from there;
- a caller compiled before its callee calls it through the callee's MethodDesc slot (`call [MethodDesc + 8]`), which
  holds a temporary entry point until the callee runs. Earlier dumps did not name such calls, which made a CALL look
  like an inlined callee (`Collision.CanHit` in `NPC.AI`: 69 calls named in the player run, 0 in the world run where
  CanHit never ran);
- the same method compiled in two processes gets the same call / inline answers almost everywhere: 3 of the 299
  callers with float locals that ran in more than one run differ (all on generic mscorlib callees:
  `List<Point>.GetEnumerator` in `MahoganyTreeBiome.Place`, `Dictionary.TryGetValue` in `NPC.SetDefaults`,
  `List<string>.Contains` in `NetMessage.SendData`); those methods keep the size rule. Every other apparent difference
  was the reading, not the JIT: unnamed slot calls (33 callers before the slot naming), calls to the accessors of a
  multi-dimensional array (`Tile[,]::Get`, a runtime stub that is not named reliably; now "unknown" when not named),
  and inlined callees whose own IL was missing from the dump (now written for two levels of callees);
- even mscorlib inlining is not order-independent: 33 (caller, mscorlib callee) pairs differ between a reference run
  and the whole-game precompile (callees first); the reference run's answer is the one used now (`jit_calls.tsv`);
- a method compiled with MinOpts still differs in its code between runs (`NPC.AI`: 549,752 against 549,541 bytes): a
  call to a callee already compiled is direct, to one not compiled yet through the slot. No float consequence (MinOpts
  keeps every local in memory).

Inlined XNA `Vector2` operators (2026-10-14, late; the entity reference runs' own code, `work\jit\ref\entities_*`;
generators `tools\cs2rs\gen_jit_vec_temps.py`, `gen_jit_vec_ctx.py`; translator Emit.cs `VecInlined` / `VecX87` /
`VecScalar` / `VecTwinSuffix`):
- **the float argument of an inlined operator** (`v * s`, `s * v`, `v / s`) is passed as it is when it is a local, a
  field, a parameter or a call's return value, and rounded through a 4-byte slot when it is any other expression (the
  importer casts an arithmetic tree to the parameter's type, a call's result is already "float" to it):
  `GolfHelper.ContactListener.OnCollision`, `collision.Normal * Vector2.Dot(velocity, collision.Normal) *
  (DirectImpactDampening - SideImpactDampening)`: the Dot result is used by `fmul st(1)` straight from the call, the
  difference goes through `fstp dword [ebp-0x148]` / `fld dword [ebp-0x148]` (golf balls 741, 742, 744). The same holds
  for the float argument of any XNA body inlined at the site (`jitBodies` templates): `WorldItem.
  TryCombiningIntoNearbyItems`, `Vector2.Lerp(worldItem.velocity, velocity, amount)` with `amount = (float)num2 /
  (float)stack` never stored (`fdivp`, then `fmul st(4)` in both inlined Lerps; the world item projectile 797 drops,
  velocity.X at tick 71);
- **where the results live is per method**: either the inlinee's `result` struct is a frame temporary, zeroed first
  (`xorps xmm0, xmm0` / `movq qword [..], xmm0`) and written per field with `fstp dword` (every operator rounds:
  OnCollision), or it is promoted and its fields stay on the x87 stack through the whole chain, rounded once where the
  caller stores (`BallCollision.ClosestPointOnLineSegment`, `lineSegment.Start + vector * num2`: `fmul st(4)`,
  `faddp st(2)`, `fstp dword [edi]`). `jit_vec_temps.tsv` has the answer of every method with inlined operator sites
  that ran (60 "stack", 113 "memory"); a promoted temporary held while a call runs is spilled as a float like any x87
  local (`point - lineSegment.Start` before the call of LengthSquared);
- **Vector2 locals of a "stack" method are promoted the same way**: `Utils.FactorAcceleration`, `Vector2 vector =
  descendOfProjectile * (num * num) / 2f / timeToInterception; return currentVelocity - vector;` is `fmul`, `fmul`, `fmul`,
  `fsubrp` with the only stores those of the returned value (NPC 426, its projectile 581's velocity.Y at tick 95). The
  translator keeps such a local's unrounded components for the reads that follow in the same block until a statement
  with a call, control flow or another write (a local whose address is taken stays a Vector2);
- **an inlined game method gets its operators decided again in every caller**: the same `FactorAcceleration` inlined
  into `Projectile.RainbowBoulder_JumpToPosition` (its first call site) calls `op_Division` twice and `op_Subtraction`
  there and inlines `op_Multiply` with a memory temporary (projectile 1047, velocity.Y at tick 26). `jit_vec_ctx.tsv`
  (the caller's IL-to-native map attributes every call instruction to a statement) lists such sites; the call there goes
  to a twin `name__vcN` translated with that caller's answers.

Consequence: a float-returning helper is either a plain field read (exact) or a real call whose result arrives unrounded; in both
cases "the call's value is the unrounded return expression" holds, and its float arguments are rounded.

## 7. Scores of three rules on all probes

A = the translator's `x87` mode (every store to a float variable rounds; arguments, returns and `(float)` casts round).
B = `x87-locals` (as A, but locals and parameters whose address is not taken never round).
C = this document (sections 2, 3, 5), as implemented in `float_model/dsl.py` + `score.py`.
The number is how many of the 2,000 inputs are reproduced; `*` marks a probe that is not exact; the last column lists the variables
C puts on the x87 stack.

```
probe                                inputs A x87          B x87-locals   C jit32         enregistered by C
P01_mul_add_store_sub                2000   1479 *         2000           2000            a x
P02_mul_add_return                   2000   2000           2000           2000
P03_compound_locals                  2000   1386 *         2000           2000            a x y
P04_compound_static_field            2000   2000           2000           2000            x y
P05_compound_instance_field          2000   2000           2000           2000            x y
P06_int_cast_mul                     2000   2000           2000           2000
P07_int_cast_mul_add                 2000   2000           2000           2000
P08_cmp_store_vs_expr                2000   144 *          2000           2000            p x y
P10_loop_accum                       2000   735 *          2000           2000            s
P11_loop_recurrence                  2000   1258 *         2000           2000            s
P12_vector2_fields                   2000   1279 *         2000           2000            v.X v.Y z
P13_vector2_ops                      2000   1236 *         2000           2000            c.X c.Y t1.X t1.Y t2.X t2.Y t3.X t3.Y x y z
P14_vector2_length                   2000   1757 *         2000           2000            num x y
P15_vector2_normalize                2000   1366 *         2000           2000            num num2 x y
P16_call_arg                         2000   2000           2000           2000
P17_double_from_float_product        2000   2000           2000           2000
P18_double_from_float_expr           2000   2000           2000           2000
P19_sqrt_float_product               2000   2000           2000           2000            r
P20_sqrt_sum_squares                 2000   2000           2000           2000            x y
P22_product_chain                    2000   2000           2000           2000
P23_assoc_residue                    2000   2000           2000           2000            x y z
P24_tile_coord                       2000   2000           2000           2000            x
P27_minmax                           2000   2000           2000           2000            z
P28_div_const                        2000   2000           2000           2000            x
P29_int_to_float                     2000   2000           2000           2000
P31_ten_locals                       2000   614 *          1062 *         2000            a0 a1 a2 a3 a4 a5 x y z
P32_local_across_call                2000   2000           1479 *         2000            b x
P33_loop_with_call                   2000   2000           1601 *         2000            s
P34_local_address_taken              2000   2000           2000           2000
B00_core_small                       2000   1147 *         2000           2000            a b x y z
B01_il_64k                           2000   2000           1147 *         2000             [MinOpts]
B02_il_32k                           2000   1147 *         2000           2000            a b x y z
B03_il_42k                           2000   2000           1147 *         2000             [MinOpts]
B03b_il_56k                          2000   2000           1147 *         2000             [MinOpts]
B10_40_hot_ints                      2000   1147 *         2000           2000            a b x y z
B11_70_hot_ints                      2000   2000           1147 *         2000
B12_140_hot_ints                     2000   2000           1147 *         2000
B13_300_hot_ints                     2000   2000           1147 *         2000
C01_use_before_and_after_call        2000   1712 *         1646 *         2000            a b x y
C02_three_refs_one_call              2000   1738 *         1517 *         2000            a x y
C03_four_refs_two_calls              2000   2000           1414 *         2000            x
C04_temp_across_call                 2000   2000           2000           2000
C05_local_then_call                  2000   2000           1616 *         2000            a
C07_inline_single_use                2000   1634 *         1634 *         2000
C08_inline_multi_use                 2000   1656 *         1656 *         2000
C09_noinline_return                  2000   1479 *         1479 *         2000
C10_inline_three_args                2000   1556 *         1556 *         2000            x
C11_loop_call_use_before             2000   1621 *         1348 *         2000            s
C12_many_refs_many_calls             2000   1967 *         1143 *         2000            a x z
C20_common_subexpression             2000   1453 *         2000           2000            a b x y z
C21_cse_and_many_locals              2000   1575 *         1658 *         1574 *          a0 a1 a2 a5 x y z
L06_six_live                         2000   1390 *         2000           2000            a0 a1 a2 a3 a4 a5
L07_seven_live                       2000   1418 *         1797 *         1767 *          a0 a1 a2 a3 a4 a5
L08_refcount_order                   2000   1254 *         1550 *         1543 *          a0 a1 a2 a3 a6 a7
L09_two_phases                       2000   1047 *         2000           2000            a0 a1 a2 a3 b0 b1 b2 b3
L10_seven_one_dead                   2000   1352 *         2000           2000            a0 a1 a2 a3 a4 a5 a6
L12_decl_reversed                    2000   1382 *         1956 *         1869 *          a0 a1 a2 a3 a5 a6
L13_param_competes                   2000   1750 *         2000           2000            a0 a1 a2 a3 a4 a5 x
L14_deep_expression                  2000   1781 *         1908 *         1905 *          a0 a1 a2 x y z
M01_array_element                    2000   2000           2000           2000            x
M02_local_struct_fields              2000   1200 *         2000           2000            v.X v.Y x y z
M03_cast_float_expr                  2000   2000           2000           2000
M03b_cast_float_local                2000   2000           2000           2000            a x
M03c_cast_float_of_local             2000   2000           2000           2000            a x
M05_int_cast_of_local                2000   1893 *         2000           2000            a
M06_ref_local_other_local            2000   1746 *         2000           2000            b x y z
S01_if_both_branches                 2000   1468 *         2000           2000            a x y z
S03_param_in_place                   2000   1522 *         2000           2000            x y z
S03b_param_assigned_once_used_once   2000   1513 *         2000           2000            x y
S04_conditional_expr                 2000   1508 *         2000           2000            a x y z
S05_cmp_local_vs_field               2000   144 *          2000           2000            a
S06_loop_two_accumulators            2000   1217 *         2000           2000            s t
S07_loop_if                          2000   1593 *         2000           2000            s x y
S08_nested_loops                     2000   1270 *         2000           2000            a s x
T01_try_assigned_in_try              2000   1074 *         2000           2000            a y
T02_try_unrelated_local              2000   1639 *         2000           2000            b
T03_local_live_through_try           2000   2000           1424 *         2000            y
probes reproduced exactly: A 33 / 77, B 52 / 77, C 72 / 77
```

Not in the table: P09, P21 (no stores at all; identical under every rule; exact in `floatprobe_model.py`), P25, P26, P30 (need the
x87 sin / cos / atan2 / pow: 2000 / 2000 each in `tests/probes.rs` of this crate), and the 16 hand-written probes of the second set
(`hand.py`: each is reproduced 2000 / 2000 by the reading given in sections 2 and 6).

## 8. Recommendation for the translator

1. Fix what does not depend on register allocation (section 2): do not round at `return`; round float arguments also for inlined
   callees; round `(float)intExpr`; keep `Math.Sin / Cos / Atan2` results as register values (`terraria-x87::*_ext`) until the next
   store or cast.
2. Decide MinOpts per method from the real assembly (IL byte count and instruction count per method are one reflection pass over the
   game exe). For MinOpts methods keep today's `x87` behaviour for locals. This covers the giant methods exactly.
3. For the other methods implement section 3. Inputs: reference counts with loop nesting, liveness (for "live across a call",
   "live into a handler" and interference), address-taken, local order. Decompiled C# local order is used only in the tie-break of
   the candidate order, which is the part that is undetermined anyway.
4. Make the translator report every method where more than six candidates interfere (section 4, items 1 and 2). Those are the
   methods where a remaining difference to the game is expected; on the probes that is 5 of 77.
5. A syntactic rule cannot be complete: the allocation depends on JIT-internal temporaries and on a tie order that the probes did not
   resolve. The practical alternative for a method that matters is to read the truth from the JIT: `probes2_code.bin` shows the
   technique (`RuntimeHelpers.PrepareMethod`, then read the machine code; a float local in memory appears as `fstp dword [ebp-N]`).
   The same can be done for any method of the game exe without running it.

## 9. Evidence from world generation (2026-10-11)

The translated generator, with the rule exactly as sections 1 - 5 state it (the translator's `jit32` mode: trees
unrounded, the allocation rule of section 3 on the decompiler's locals, the one `floatInMemory` exception of
section 4, `System.Math` through this crate in its plain double forms), reproduces the real game's world for seed
"12345", small: all 106 passes with identical tile maps and identical next random numbers, and a saved world
file that is identical apart from GUID and time stamps (`tools/cs2rs/VERIFICATION.md`, Proof 2). Nothing new
about the rule was learned from it: no pass needed a float return left unrounded, a float parameter, a promoted
struct field, a register (`_ext`) form of a trigonometric function or another `floatInMemory` entry.

What that does and does not say: world generation is long (about 10^10 loop iterations) but most of its float
arithmetic feeds comparisons and casts to `int`, where a difference of one ulp only matters at a threshold. One
world passing is strong evidence that the rule is right where it is determined, and no evidence about the open
parts (section 4): another seed or size can still sit on such a threshold. The tool to read the real JIT's
decision for a method (`float_model/disasm.py` on jitted code) was not extended; it is the next step only when a
comparison diverges.

## 10. Evidence from the entity reference runs (2026-10-14; client-side item use added the same day)

`refdata/entities` runs every NPC type and every projectile type (and, since the same day, every usable item) on the
real dedicated server for 120 ticks; `tools/refdump_worldgen/run_refjit.sh` repeats those runs byte for byte and dumps
their machine code. Rules read there (each with the type whose comparison showed it; tools/cs2rs/VERIFICATION.md,
"Entities"):

1. **The JIT's own allocation is in the CLR's debug info.** The code header's `[entry-16]` points at the debug info the
   CLR keeps for every jitted method even without a debugger (`JitDump.cs WriteDebugInfo`, `jit_debuginfo.py`): for
   every IL argument and local its native ranges and location, `VLT_STK` (a frame slot: every store rounds) or
   `VLT_FPSTK` (an x87 register: stores do not round). Over the 4,994 methods that ran in the 22 reference runs this
   replaces the rule of section 3 wherever the method's IL in Terraria.exe equals the server's, tokens aside (388 methods with float
   locals: 3,367 locals in memory, 286 on the x87 stack, 380 without a range). Decoding note: a frame offset is stored
   as `(offset / 4) << 1` in 31 bits.
2. **A decompiled variable that is not an IL local is an evaluation-stack value.** The C# compiler keeps single-use
   values on the IL stack (`num22 = (float)Main.rand.Next(26, 301) * 0.01f` in `NPC.AI_065_Butterflies`, used after two
   more `Next` calls); ILSpy turns them into locals. The JIT keeps such a value unrounded and spills it to an 8-byte
   slot around calls (`fstp qword [ebp-0x160]`): never rounded. Which decompiled variables are IL locals: the portable
   PDB that ILSpy writes for the same decompilation (`tmeta pdblocals`). Section 4, item 8 is resolved by this.
3. **A float return value stays unrounded in the caller** (section 2) for game methods too: `scale = Opacity / 2f *
   localAI[0]` with `Opacity => 1f - (float)alpha / 255f` (projectile 658) and `Opacity += 0.1f` (projectile 961, alpha
   151 not 150). The translator gives every directly called float method an unrounded twin.
4. **`Math.Abs(float)` compiled as a call rounds its argument.** A method compiled without optimisation inlines nothing
   (`NPC.FindFrame`: 113 calls to `Math.Abs(float)` at 113 sites), so `frameCounter += Math.Abs(velocity.X * 1.1f)` adds
   the product rounded to float (NPC 243). Where the JIT inlines it (`fabs`), the argument stays unrounded (section 2).
5. **`0f - x` in the decompiled text is `neg` (`fchs`)**: the sign of a zero flips (`rotation = (0f - velocity.X) *
   0.05f` is -0.0 for velocity.X = 0; projectiles of aiStyle 14). Every `0f - x` / `0.0 - x` of the 4,994 methods that
   ran has its IL `neg`. XNA's `Vector2` negation is `fchs` too (`-Vector2.UnitY` = (-0, -1); projectile 601).
6. **Compound bodies of XNA members keep their intermediates** (their own machine code): `Vector2.Lerp`, `Vector3.Lerp`,
   `MathHelper.Lerp` (result unrounded in ST0), `SmoothStep` (both), `Vector2 / float` (`1 / d` unrounded), `Distance`
   (differences unrounded, the root rounded), `DistanceSquared`, `Dot`, `LengthSquared` (results unrounded),
   `Normalize()`, `CatmullRom`, `Reflect` (the dot product unrounded). Where the JIT inlined such a member the same
   x87 code runs inside the caller (one of the two `Vector2 / float` in `NPC.AI_111` is inlined; NPC 578).
7. **`x op= tree` on an array element or a property is one x87 expression** (`localAI[0] += (Math.Abs(velocity.X) +
   Math.Abs(velocity.Y)) * 0.01f * (float)direction`: the falling star's localAI[0] at tick 3 is 0.21 = 0x3e570a3d; with
   the tree rounded first it is 0x3e570a3e).
8. **Inlined `Vector2` operators** (section 6, "Inlined XNA Vector2 operators"): the scalar argument enters unrounded
   when it is a local, field, parameter or call result; the operator results stay on the x87 stack or are rounded per
   operator as the method's own code shows (jit_vec_temps.tsv), promoted Vector2 locals of such a method too; an inlined
   game method's operators are decided per caller (jit_vec_ctx.tsv, `__vcN` twins). With these rules every NPC (696),
   projectile (1,135) and server-side item use (4,629) is bit-exact: golf balls 741 / 742 / 744 (OnCollision: the Dot
   result unrounded into the inlined multiplication), projectile 797 (the inlined Lerp's `amount`), NPC 426 (the
   promoted chain of FactorAcceleration), and projectile 1047, which the per-method rule alone broke (FactorAcceleration
   inlined in RainbowBoulder_JumpToPosition with its operators called).
9. **Virtual calls returning float** keep their rounded result in the translation (the `__x87` twin is for direct calls).
   None of the 24 virtual / override / abstract float members of the game (sky alpha, creative power sliders, shield
   values, animation progress) ran in any of the 24 reference processes, so no reference shows whether it matters.
10. **A float call result waiting while another call runs is spilled as a float** (`fstp dword`), unlike an arithmetic
   temporary (C04: `fstp qword`): the JIT takes a call's float result for an exact float. `Projectile.AI_165_Whip`,
   `Utils.GetLerpValue(0.1f, 0.7f, t3, true) * Utils.GetLerpValue(0.9f, 0.7f, t3, true) > 0.5f`: `call`, `fstp dword
   [ebp-0xa4c]`, `call`, `fmul dword [ebp-0xa4c]` at all 11 such pairs of the method, in the client run below and in
   the projectile run (whips 5479 / 5480 of the client item scenario: one `Main.rand` draw more at tick 12 without it).
   Read from the client-side item reference run (`refdata/entities/citem.*`, `work/jit/ref/entities_citem`: the owning
   player uses every usable item; 3,484 methods ran, among them the client's item code no earlier run executed). With
   that run's facts added to the others (24 runs) and this rule, all 4,629 item types are bit-exact on the owning
   client too.
11. **A callee the JIT inlined at some sites and called at others is decided site by site** (draw reference,
   `work/jit/client/draw`, 2026-10-14): `Dust.GetAlpha` calls `Color.get_R` at 2 of its 14 sites (`new Color(newColor.R,
   ..)`) and inlines it in `(int)((float)newColor.R * num)`, where `num` stays on the x87 stack (`fmul st(1)`). Each call
   instruction -> its IL offset (the CLR's IL / native map) -> its decompiled statement (ILSpy's PDB) and ordinal there:
   `tools/cs2rs/jit_refcall_sites.tsv`, FloatAlloc.cs `SiteIsCall`. Before, such a pair fell back to the size rule
   (a call everywhere), rounding `num` before each channel product (red 0xad instead of 0xae).
12. **Vector2 products whose calls all come first stay on the x87 stack** (draw reference, `VertexStrip.AddVertexPair`):
   `vector = WrapAngle(..).ToRotationVector2() * num` with op_Multiply inlined keeps both products unrounded through
   `AddVertexPair(pos + vector, pos - vector, ..)` (`fadd st(2)` / `fsubrp st(3)`, each argument stored with `fstp
   dword [esp]`). Emit.cs `VecCallsOnlyFirst` / `VecArgsBeforeCall`.
13. **A float local without any location in the debug info is a JIT "stack" local** (audio and scenes reference runs,
   `work/jit/client/audio`, `.../scenes`, 2026-10-15). The CLR's variable info names no home for some tracked float
   locals ("none" before). Two kinds hide there: untracked frame locals (address taken, or beyond the tracked-locals
   limit of a method with many locals: rounded) and locals the JIT keeps on the x87 stack without reporting them.
   `NextHorizonRenderer.OriginalColorsForCloud` keeps both of its locals on the x87 stack and stores neither
   (`fld st(0)`, `fmul`, ..., `fmulp st(2)`, `fstp qword` only before `cvttsd2si`), so a cloud's red channel was
   0x10 against the port's 0x11. Such a local is spilled as a float across a call like any "stack" local:
   `Projectile.AI_186_PrincessWeapon` `num7` around `Utils.Remap` (`fstp dword [ebp-0x1bc]`), which an
   evaluation-stack treatment (qword spill) broke (projectile 950, localAI[0] at tick 51). gen_jit_float_alloc.py
   writes kind `temp` for a "none" local that is not address taken (no `ldloca`) in a method with at most 48 IL locals
   and arguments; FloatAlloc.cs treats `temp` as `stack` (664 locals with the scenes run).
14. **An inlined `Vector2 / constant` multiplies by the reciprocal folded at float precision** (scenes run,
   `Star.Update`: `position += fallSpeed * f / 100f` is `fmul dword ptr [const]` with `1f / 100f` rounded to a float).
   A falling star's trail was 1 ulp off at frame 3047. Emit.cs `VecCombine`: a constant divider gives
   `((1f32 / d) as f64)`; a variable one keeps the unrounded `1.0 / d` (item 6). 37 generated files changed.
15. **A second form of the per-operator frame temporary** (scenes run, `Rain.Update`): an inlined operator result is
   written per field into a frame struct addressed by `lea reg, [ebp - N]` (no zeroing first), then used from its
   slot by x87 arithmetic (`fadd dword ptr [ebp - 0x18]`): `dust.velocity += -velocity * 0.025f` rounds the
   product. gen_jit_vec_temps.py counts that form as `memory` too (6 methods changed: Rain.Update,
   LittleFlyingCritterParticle.Update, IngameOptions.DrawLeftSide, Mount.UpdateFrame_GolfCart,
   Player.UpdateMinionTarget, Projectile.UpdateEnchantmentVisuals). A looser test (any `lea` + two `fstp dword`)
   would have flipped 33 methods, among them `BallCollision.ClosestPointOnLineSegment`, whose code is the "stack"
   example of section 6.
16. **Known history dependence, not changed**: jit_calls.py `call_answers` counts an inlined callee whose own callee is
   called as a call at all its sites, but it knows the callee's own callees only when some caller of the process
   compiled the callee on its own. The scenes run compiled `Color(Vector3)` on its own, so `Lighting.GetColor` (whose
   code is identical in every run) became a `Color(Vector3)` call site in jit_bodies.tsv. Restricting the promotion
   to non-XNA callees changed 99 jit_bodies rows and did not change the output of any scene; left as it is.
17. **The trigonometric instructions are the processor's own** (gallery and interaction runs, 2026-10-16). `fsin`,
   `fcos`, `fptan` and `fpatan` are microcode: the model of `trig.rs` (correct rounding) and the reference machine
   differ in the last bit of the 64-bit register for about 1 % of the arguments, and dust velocities through
   `Utils.RotatedBy` showed it. On x86 / x86_64 the port executes the instructions (`hw.rs`, inline assembly);
   `TERRARIA_X87_TRIG=model` and builds without x87 (the browser) keep the model. The precision control does not
   apply to these instructions (Intel SDM vol. 1, 8.1.5.2).
18. **An inlined `new Vector2(a, b)` operand of a "stack" method is its two components** (gallery run,
   `BatsGroupSkyEntity.UpdateVelocity`): with the constructor inlined (jit_bodies.tsv: seen, no call) the operands
   are not stored first. Emit.cs `VecX87`.
19. **`Vector2.Hermite`, both overloads, as XNA's own machine code computes them** (gallery run, gen_native_xna.py):
   the ref / out overload (ItemTransferParticle.Draw) keeps the amount and all six weights at double precision and
   rounds only the two components; the value overload (Projectile.DoPalAppearEffect) does the same except the first
   weight, `2a^3 - 3a^2 + 1`, which it stores to a frame float (`fstp dword [ebp-4]`). A pet's appear dust was 1 ulp
   off in position and 2e-5 in velocity with the f32 arithmetic of terraria_core.
20. **Generic arguments in method names name the server's assembly**: the server runs' `jit_debug.bin` and
   `jit_code.bin` write `List`1[[.., TerrariaServer, Version=..]]`, their jit_code.json reads `Terraria` after
   jit_calls.load_json. The fact generators matched the JSON against the binary names, so every method with a
   generic argument type that ran only in server runs (LightningGenerator.GenerateBolt, 1,053 IL bytes) had no
   float-allocation facts: jit_debuginfo.canonical now gives both the same names (+50 methods with facts).
21. **An evaluation-stack value spilled across a call is a float when it is a float call result** (gallery run,
   `LightningGenerator.GenerateBolt`: `float num7 = 0f - Vector2.Dot(..)` stays on the IL stack across `Math.Min` /
   `Math.Max` and the JIT stores it with `fchs; fstp dword [ebp-0x8c]`). The spill temporary has the type of the
   tree: a call returning float (optionally negated) is float, arithmetic is double (`AI_065_Butterflies num22 =
   (float)Next(..) * 0.01f`: `fstp qword [ebp-0x160]`). FloatAlloc.cs `FloatCallValue`: such a variable is rounded
   where it is live across a call, like a "stack" local; other evaluation-stack values stay unrounded.
22. **Only an 8-byte zeroing is a Vector2 operator temporary** (gallery run): gen_jit_vec_temps.py counted any
   `xorps xmm0, xmm0; movq qword [reg], xmm0` as one, but a 12-byte `Nullable<Vector2>` (`add reg, 8; stosd`,
   FinalFractalProfile.StripDust) and a 16-byte List enumerator (`movq qword [reg + 8], xmm0`,
   LightningGenerator.Generate) are zeroed the same way. Both methods keep their inlined operator products on the
   x87 stack; the Zenith's trail dust was 2 ulp off in velocity. 74 methods changed from memory to stack.
23. **A promoted Vector2 parameter kept on the x87 stack** (gallery run, `LightningGenerator.Generate`: `direction =
   direction.RotatedBy(..) * Length` is never stored; both products stay in registers until the inlined
   `targetPosition - direction`). The translation stores the variable as a float pair. Not a rule yet: the method's
   body is replaced (game.json `bodies`, with the machine code as evidence); a storm lightning's end point was 1 ulp
   off in Y.
24. **`array.Length` is not a call** (gallery run, `Utils.MultiLerp`): a one-dimensional array's length is `ldlen`.
   FloatAlloc.cs counted the property getter as a call (System.Array is not a translated type), so `while (percent /
   num2 > 1f && num3 < floats.Length - 2)` looked like a call in a loop header and demoted `num` and `num2` to memory
   although the machine code keeps both on the x87 stack (`fadd st(1), st(0)`). A shock icon's scale was 1 ulp off
   (frame 851). 23 generated files changed.
25. **Only the innermost binary operation decides whether a stack local waits across a call** (gallery run,
   `SolarSky.Activate`: `num * ((float)Main.maxTilesX * 16f) + NextFloat() * 40f - 20f` computes the product first and
   keeps it in an 8-byte slot across `InternalSample`, `fstp qword [ebp-0x18]`). Emit.cs `PendingAcrossCall` walked up
   through every enclosing binary operation and rounded `num` because an outer one calls on its right; that is C04
   (an expression temporary, never rounded), not C05 (`a + F(z)`, the local itself waits: rounded). A solar
   eclipse sky meteor was 32 ulp off in x (frame 2845).
26. **A float local passed to an inlined game method enters it unrounded** (world generation, the frozen world's dual
   dungeons, 2026-10-16). `DungeonShapes.HourglassRoom.Perform`: `float percent = ((float)i + (float)num) / (float)_height`
   stays on the x87 stack and `Utils.WrappedLerp(0f, 1f, percent)` is inlined: `fmul dword [2.0]` acts on the register,
   no `fstp dword` of `percent` first (work/jit/ref/wg_ref_sec_frozen, tools/refdump_worldgen/jit_disasm.py). The JIT
   substitutes a caller's local for an inlinee parameter the inlinee never writes (no `starg`, no address taken), so the
   inlinee reads the unrounded value; an argument that is an expression is still stored to a float temporary first
   (I02, I05). With the argument rounded the row of 26/28 gets `2 - 2 * 0.92857143f`, MultiLerp's `percent / num2 > 1`
   (both on the x87 stack, item 24) is true once more, the room's half-width at that row is 16 instead of 17 and the
   two edge cells of four hourglass rooms were not cleared (`ref_sec_frozen` row 30: cells 3031 / 3065 at y 459 and
   1108 at y 586; the cracked brick at 2775, 706 follows). Fact: tools/cs2rs/gen_jit_inlined.py writes
   jit_inlined.tsv, the (caller, game callee with a float parameter) pairs whose machine code has no call instruction
   to the callee ITSELF at any site in every run that ran the caller (unlike jit_refcalls.tsv, an inlined callee whose
   body still calls something counts as inlined here); 206 pairs with the coverage run. Translator: config `jitInlined`; Model.cs
   `ArgTwinParams` (the float parameters never written, address-taken or captured), Gen.cs emits `name__u` (those
   parameters f64, the body's float-local rules applied to them as "stack" variables, FloatAlloc `extraStack`) and
   `name__u__x87`; Emit.cs `InlinedArgsTwin` calls the twin where a pair is inlined and at least one such argument is a
   float local of the caller kept on the x87 stack (not an evaluation-stack value). 39 call sites, 51 twin functions.
27. **XNA's matrix kernels sum their products on the x87 stack** (the coverage run, work/jit/client/cover, 2026-10-16).
   `Vector2.Transform(Vector2, Matrix)` is `fld; fmul; fld; fmul; faddp; fadd; fstp dword` per component, one rounding
   at the store; `Vector3.Transform` and `Matrix * Matrix` the same; `Matrix.CreateRotationZ` computes `fcos` / `fsin`
   (item 17) and stores each as a float, `M21` by `fchs`. terraria_core evaluated them in f32 arithmetic (a rounding
   per operation) with the correctly rounded `cos` / `sin`: a UFO mount's sprite was 1 ulp off in y at frame 1294
   (`PlayerDrawLayers.DrawPlayer_TransformDrawData`, the player's full rotation). Config `jitBodies` now has the four
   (game.json, with their machine code as the evidence).
28. **`Math.Max` / `Math.Min` of a NaN return the NaN operand itself** (the probe run, `GolfHelper` shot strength,
   2026-10-17). .NET Framework 4's `Math.Max(float, float)` is `if (val1 > val2 || float.IsNaN(val1)) return val1;
   return val2;` (and `Math.Min` with `<`; the double overloads the same): a NaN argument comes back with its own bits,
   the sign included, where IEEE `maxNum` and Rust's `f32::max` return the other operand. The probe's golf shot gave
   `ffc00000` (the x87 default NaN, negative) in the original and `7fc00000` in the port. `rt::math::max_f32` /
   `min_f32` / `max_f64` / `min_f64` now follow the Framework's code.
29. **The JIT dumps include closed instantiations of the game's generic types and methods** (2026-10-17). The dumper
   (tools/refdump_worldgen/JitDump.cs `ClosedGenericTypes`, `ClosedGenericMethods`) resolves the game assembly's
   TypeSpec and MethodSpec tables and checks each instantiation's methods like any other; a dump names them
   `Definition`n<Arg1,Arg2>::Method` and `Type::Method<Arg1>`, so float_coverage.py credits the definition while the
   fact generators (keyed by exact names) never apply one instantiation's machine code to another. Before this, no
   method of a generic type could be covered at all (`WeightedRandom<Color>.Get`, `GroupOptionButton<bool>.DrawSelf`,
   `UITextPanel<T>.SetText`; a generic method over reference types needed item 34).
30. **The fact chain runs twice** (2026-10-17). gen_jit_vec_temps.py reads jit_bodies.tsv and gen_jit_refcalls.py
   reads jit_calls.tsv, both written later in the chain; after new dumps one pass leaves facts that depend on the
   previous run's files. A probe-run sky entity (`AmbientSky.EOCSkyEntity.BeginChasingPlayer`) had no Vector2-temporary
   fact after one pass and was 1 ulp off in y; the second pass gave it `stack` and changed nothing else (a third
   pass leaves all eight tables unchanged, checked 2026-10-17 on the probe run's dumps). Regenerating the facts: gen_jit_refcalls, gen_jit_float_alloc, gen_jit_vec_temps,
   gen_jit_vec_ctx, gen_jit_bodies, gen_jit_calls, the same six again, then tools/cs2rs/run.sh.
31. **A struct's method reached only through an interface was not in the dump** (2026-10-17). `TorchID.TorchColor` ran
   in the probe run for all 24 torch styles, through `ITorchLightProvider.GetRGB`, yet none of the struct lights
   (`ConstantTorchLight`, `DemonTorchLight`, `DiscoTorchLight`, `ShimmerTorchLight`) was listed as run: reflection's
   entry of a struct method is its unboxing stub (`add ecx, 4; jmp [slot]`), and the code the slot leads to names the
   method's other MethodDesc (eight bytes on) in its header. The dumper (JitDump.cs `EntryNoJit`, `DumpRan`) now
   follows the stub and accepts that descriptor; the next probe run listed all four.
32. **XNA's by-reference `Vector2.Transform(ref, ref, out)` sums on the x87 stack as the by-value one does** (the
   probe run, 2026-10-17). Inlined in `DynamicSpriteFont.InternalDraw` (work/jit/client/probe): `fld; fmul; fld;
   fmul; faddp; fadd` per component, both components computed before the two `fstp dword`. The overload was still
   mapped to terraria_core's f32 `Vector2::transform` (a rounding per operation): a rotated crit number's glyph
   (`CombatText`, the probe's minecart group) was 1 ulp off in x. Config `jitBodies` now has the overload too.
33. **A float local initialised with a constant and never written again is a constant to the JIT** (the probe run,
   2026-10-17). `ParticleOrchestrator.Spawn_FlameWaders`: `float num = 60f; ... Vector2.One * num3 * (-1f / 60f) / num`;
   the inlined `Vector2 / float` folds `1f / num` at float precision and multiplies by the 32-bit constant (`fld dword
   ptr [const]`, `fmul st(1)`), as item 14 found for a literal divider. The port divided by the unrounded reciprocal: the
   flame particles' scale (`FlameParticle`, drawn with the greek fire textures) was 1 ulp off. Emit.cs `JitConstLocal`
   (no assignment, increment, ref / out / address use or capture after the initialiser; not in a MinOpts body) now
   takes such a local as the literal.
34. **The shared code of a generic method over reference types is found through its callers** (2026-10-17).
   `UICreativePowersMenu.CreateSubcategoryButton<T>` (T a class) has one body for every T, whose MethodDesc is the
   instantiation over `System.__Canon`; the callers pass the exact instantiation as a hidden argument and call that
   code directly, so the exact MethodDescs' precodes never run and no candidate of the dump led to it. JitDump.cs
   `AddSharedGenericCode` takes the call targets of the code that ran, follows their precode jumps, reads the
   MethodDesc in the code header, checks that its chunk belongs to a MethodTable of the dumped assemblies (so a helper's
   stray bytes are never handed to the runtime) and names it with `RuntimeType.GetMethodBase(RuntimeType,
   RuntimeMethodHandleInternal)`: `Type::Method<System.__Canon>(...)`, which float_coverage.py credits to the
   definition.
35. **XNA's instance `Vector3.Normalize()` is Vector2's with a third square** (the probe run, 2026-10-17). Its code
   (work/jit/client/probe): the three squares summed on the x87 stack, `fsqrt`, the root stored as a float, `fld1;
   fdivrp` (the reciprocal unrounded), each component multiplied and stored. terraria_core normalised in f32 arithmetic:
   `ReflectiveArmorShaderData.Apply`'s light direction (`uLightSource`, a reflective dye on a body armour) was 1 ulp off
   in y. Config `jitBodies` now has it.
