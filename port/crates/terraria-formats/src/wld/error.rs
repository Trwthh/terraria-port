//! Errors of the world file reader/writer. Every read error carries the section and the byte offset.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum WldError {
    /// The file ended while reading `wanted` more bytes.
    UnexpectedEof { section: &'static str, offset: usize, wanted: usize },
    /// The file version needs a loader branch this reader does not implement. `branch` names the first one.
    UnsupportedVersion { version: i32, branch: &'static str },
    /// `LoadWorld_Version2` returns `StatusID.BadSectionPointer`: the stream position after `section`
    /// differs from the pointer stored in the header.
    BadSectionPointer { section: &'static str, expected: i64, actual: i64 },
    /// `LoadFooter` returns `StatusID.BadFooter`.
    BadFooter { offset: usize, reason: &'static str },
    /// Data that the original loader would reject, crash on, or skip without consuming.
    Invalid { section: &'static str, offset: usize, reason: String },
    /// The `World` value cannot be written the way the original writer would write it.
    Unrepresentable { section: &'static str, reason: String },
}

impl fmt::Display for WldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WldError::UnexpectedEof { section, offset, wanted } => {
                write!(f, "{section}: unexpected end of file at offset {offset} (wanted {wanted} more bytes)")
            }
            WldError::UnsupportedVersion { version, branch } => {
                write!(f, "unsupported world file version {version}: first unsupported branch is `{branch}`")
            }
            WldError::BadSectionPointer { section, expected, actual } => {
                write!(f, "bad section pointer after {section}: header says {expected}, stream is at {actual}")
            }
            WldError::BadFooter { offset, reason } => write!(f, "bad footer at offset {offset}: {reason}"),
            WldError::Invalid { section, offset, reason } => {
                write!(f, "{section}: invalid data at offset {offset}: {reason}")
            }
            WldError::Unrepresentable { section, reason } => write!(f, "{section}: cannot write: {reason}"),
        }
    }
}

impl std::error::Error for WldError {}
