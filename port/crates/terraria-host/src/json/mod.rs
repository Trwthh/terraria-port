//! Newtonsoft.Json 10 (the game's own copy, `meta\refs\Newtonsoft.Json.dll`) for the translated game: the `Linq`
//! DOM ([`token::JToken`]), the reader and writer ([`reader`], [`writer`]), number text ([`number`]), conversions to
//! and from .NET values ([`convert`]) and BSON ([`bson`]).
//!
//! Not here (needs type metadata only the translator has): reflection-driven `DeserializeObject<T>` /
//! `SerializeObject` of game classes. REQUESTS.md, section "Newtonsoft.Json", lists the game types and the member
//! table the generator has to emit; [`convert::JsonClass`] is the host side of that table.
//!
//! The runtime (`terraria-gen-rt`) may carry a minimal reader for the localisation files; `Dictionary<string,
//! Dictionary<string, string>>` can be mapped to [`convert::deserialize_string_map`] instead.
//!
//! Known deviations from the original:
//! - an ISO date with an explicit offset (`2020-01-01T10:11:12+01:00`) is kept with that offset; the original converts
//!   it to the machine's local time zone and writes the local offset;
//! - positions in error messages count `char`s, not UTF-16 units;
//! - a lone surrogate escape (`\ud800`) becomes U+FFFD (strings of this runtime are UTF-8).

pub mod bson;
pub mod convert;
pub mod number;
pub mod reader;
pub mod token;
pub mod writer;

#[cfg(test)]
pub(crate) mod tests;
