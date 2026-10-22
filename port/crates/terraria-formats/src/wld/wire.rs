//! A tiny trait so a struct can be declared once and get its reader and writer generated in field order.
//! Field order in the declaration IS the file order, so each declaration is a direct transcription of the
//! original `Save*`/`Load*` statement sequence.

use super::binio::{Reader, Writer};
use super::error::WldError;

pub(crate) trait Wire: Sized {
    /// Smallest number of bytes one value takes (used to bound counts read from the file).
    const MIN_SIZE: usize;
    fn read(r: &mut Reader) -> Result<Self, WldError>;
    fn write(&self, w: &mut Writer) -> Result<(), WldError>;
}

macro_rules! wire_prim {
    ($t:ty, $size:expr, $r:ident, $w:ident) => {
        impl Wire for $t {
            const MIN_SIZE: usize = $size;
            fn read(r: &mut Reader) -> Result<Self, WldError> {
                r.$r()
            }
            fn write(&self, w: &mut Writer) -> Result<(), WldError> {
                w.$w(*self);
                Ok(())
            }
        }
    };
}

wire_prim!(u8, 1, u8, u8);
wire_prim!(i16, 2, i16, i16);
wire_prim!(u16, 2, u16, u16);
wire_prim!(i32, 4, i32, i32);
wire_prim!(u32, 4, u32, u32);
wire_prim!(i64, 8, i64, i64);
wire_prim!(u64, 8, u64, u64);
wire_prim!(f32, 4, f32, f32);
wire_prim!(f64, 8, f64, f64);
wire_prim!(bool, 1, bool, bool);

impl Wire for String {
    const MIN_SIZE: usize = 1;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        r.string()
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        w.string(self);
        Ok(())
    }
}

/// Fixed-size run of values, no count (`treeX[0..3]`, `ReadBytes(16)`, ...).
impl<T: Wire, const N: usize> Wire for [T; N] {
    const MIN_SIZE: usize = T::MIN_SIZE * N;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        let mut v = Vec::with_capacity(N);
        for _ in 0..N {
            v.push(T::read(r)?);
        }
        match v.try_into() {
            Ok(a) => Ok(a),
            Err(_) => Err(r.invalid("fixed array length")),
        }
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        for x in self {
            x.write(w)?;
        }
        Ok(())
    }
}

/// `Write(int count)` followed by the values (`anglerWhoFinishedToday`, `TempPartyCelebratingNPCs`).
impl<T: Wire> Wire for Vec<T> {
    const MIN_SIZE: usize = 4;
    fn read(r: &mut Reader) -> Result<Self, WldError> {
        let n = r.count(T::MIN_SIZE, "list")?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(T::read(r)?);
        }
        Ok(v)
    }
    fn write(&self, w: &mut Writer) -> Result<(), WldError> {
        w.len_i32("WorldHeader", self.len(), "list")?;
        for x in self {
            x.write(w)?;
        }
        Ok(())
    }
}

/// Declares a plain struct whose reader and writer visit the fields in declaration order.
macro_rules! wire_struct {
    ($(#[$sm:meta])* pub struct $S:ident { $( $(#[$fm:meta])* pub $f:ident : $t:ty ),* $(,)? }) => {
        $(#[$sm])*
        #[derive(Clone, Debug, PartialEq, Default)]
        pub struct $S { $( $(#[$fm])* pub $f: $t ),* }
        impl $S {
            #[allow(clippy::needless_update)]
            pub(crate) fn read_fields(r: &mut Reader) -> Result<Self, WldError> {
                Ok($S { $( $f: <$t as Wire>::read(r)?, )* })
            }
            pub(crate) fn write_fields(&self, w: &mut Writer) -> Result<(), WldError> {
                $( <$t as Wire>::write(&self.$f, w)?; )*
                Ok(())
            }
        }
    };
}
pub(crate) use wire_struct;
