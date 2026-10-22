// The original file reads its tables below env!("CARGO_MANIFEST_DIR")/../../../refdata. Under WASI the project root is
// preopened as /w, so this crate shadows env! (textual macro scope wins over the built-in) to give a guest path whose
// ../../.. lands on /w/refdata. The test file itself is not copied or changed.
macro_rules! env { ($n:literal) => { "/w/port/crates/terraria-x87" } }
#[path = "../../../../port/crates/terraria-x87/tests/probes.rs"]
mod probes;
