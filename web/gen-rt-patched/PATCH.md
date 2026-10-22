Copy of `port/crates/terraria-gen-rt` (2026-10-13). One change: `src/rt/gc.rs` `Marker::mark_conservative_stack`
(port line 730) — the address filter `0x1_0000..0x8000_0000_0000` does not fit a 32-bit `usize` (compile error
`overflowing_literals` on wasm32); here it is chosen by `target_pointer_width`. Nothing else differs. Used only
to run the runtime's own tests on wasm32-wasip1 (results in web/logs/gen-rt-patched-*.log).

Second change (test code only): `gc.rs` test `poison_mode_makes_stale_handles_visible` (port line 2725 before the first
patch) compares with `0xDDDD_DDDD_DDDD_DDDDusize`; here `usize::from_ne_bytes([0xDD; size_of::<usize>()])`.

Third change: `gc.rs` `Marker::mark_conservative` (port line 698) skips every word that is not a multiple of 8
(`w % 8 != 0`). On wasm32 a `Str` inside a suspended iterator method's state machine points 4 bytes past an 8-byte
boundary, so the string was freed under the machine (test `rt::iter::tests::suspended_machine_keeps_its_locals_alive`
fails, reading poisoned bytes). Here: `w % word` (`word = size_of::<usize>()`). With it the test passes.
