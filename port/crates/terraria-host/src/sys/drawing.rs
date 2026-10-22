//! `System.Drawing` as far as the game calls it: the value types `Point`, `Size`, `Rectangle` (window and monitor
//! geometry in `Main.SetDisplayMode` / `WindowStateController`) and the `Bitmap` the screenshot code saves through
//! (`Graphics/Capture/CaptureCamera.cs`: `new Bitmap(w, h)`, `LockBits(rect, WriteOnly, Format32bppPArgb)`,
//! `Marshal.Copy(data, 0, bitmapData.Scan0, n)`, `UnlockBits`, `Save(path, ImageFormat.Png)`).
//!
//! The game only ever asks for `ImageFormat.Png` (decompile grep: the `imageFormat` parameters receive
//! `ImageFormat.Png` at both call sites). JPEG, BMP, GIF and TIFF are `unimplemented` with that reason.
//! `Bitmap.Save` writes through `sys::io`, so the install folder stays read-only.
//!
//! Pixel format: `Format32bppPArgb` is B, G, R, A bytes with premultiplied colour. PNG stores straight alpha, so
//! `Save` divides the colour by alpha (rounded), as GDI+ does on conversion (rounding of GDI+ itself not verified).

use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, Arr, Gc, Str};

use crate::gfx::states::{plain_trace, throw};

/// `System.Drawing.Point`
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
/// `System.Drawing.Size`
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}
/// `System.Drawing.Rectangle`
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
plain_trace!(Point, Size, Rectangle);

impl Point {
    pub fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
}
impl Size {
    pub fn new(width: i32, height: i32) -> Size {
        Size { width, height }
    }
}
impl Rectangle {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Rectangle {
        Rectangle { x, y, width, height }
    }
    pub fn right(&self) -> i32 {
        self.x + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
}

/// `System.Drawing.Imaging.ImageFormat` (the static instances only): `Png` 0, `Jpeg` 1, `Bmp` 2, `Gif` 3, `Tiff` 4.
pub struct ImageFormat {
    pub kind: i32,
}
plain_trace!(ImageFormat);
impl ImageFormat {
    pub fn get(kind: i32) -> Gc<ImageFormat> {
        // one object per format, like the original's static properties
        thread_local! {
            static FORMATS: std::cell::RefCell<Vec<Gc<ImageFormat>>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        FORMATS.with(|f| {
            let mut f = f.borrow_mut();
            if f.is_empty() {
                for k in 0..5 {
                    let g = rt::alloc_init(ImageFormat { kind: k });
                    rt::add_root(move |m| m.mark(g));
                    f.push(g);
                }
            }
            f[kind as usize]
        })
    }
}

/// `System.Drawing.Imaging.BitmapData`: where the locked pixels are.
pub struct BitmapData {
    pub scan0: isize,
    pub stride: i32,
    pub width: i32,
    pub height: i32,
}
plain_trace!(BitmapData);

/// `System.Drawing.Bitmap` (32 bits per pixel, premultiplied alpha).
pub struct Bitmap {
    pub width: i32,
    pub height: i32,
    /// B, G, R, A rows
    pub bgra: Vec<u8>,
    locked: bool,
    disposed: bool,
}
plain_trace!(Bitmap);

impl Bitmap {
    pub fn new_gc(width: i32, height: i32) -> Gc<Bitmap> {
        if width <= 0 || height <= 0 {
            throw("System.ArgumentException", "Parameter is not valid.");
        }
        rt::alloc_init(Bitmap { width, height, bgra: vec![0; width as usize * height as usize * 4], locked: false, disposed: false })
    }
    /// `LockBits(rect, mode, format)`: the pixel format must be `Format32bppPArgb` (0xE200B) or `Format32bppArgb`
    /// (0x26200A); the whole bitmap is addressable through `Scan0`.
    pub fn lock_bits(&mut self, rect: Rectangle, _mode: i32, pixel_format: i32) -> Gc<BitmapData> {
        if self.disposed {
            throw("System.ArgumentException", "Parameter is not valid.");
        }
        if pixel_format != 0xE200B && pixel_format != 0x26200A {
            host::unimplemented("System.Drawing.Bitmap.LockBits(Rectangle, ImageLockMode, PixelFormat) with a pixel format other than Format32bppPArgb / Format32bppArgb");
        }
        if rect.x < 0 || rect.y < 0 || rect.right() > self.width || rect.bottom() > self.height || rect.width <= 0 || rect.height <= 0 {
            throw("System.ArgumentException", "Rectangle must be inside the bitmap.");
        }
        if self.locked {
            throw("System.InvalidOperationException", "Bitmap region is already locked.");
        }
        self.locked = true;
        let stride = self.width * 4;
        let scan0 = self.bgra.as_mut_ptr() as isize + (rect.y as isize * stride as isize + rect.x as isize * 4);
        rt::alloc_init(BitmapData { scan0, stride, width: rect.width, height: rect.height })
    }
    pub fn unlock_bits(&mut self, _data: Gc<BitmapData>) {
        self.locked = false;
    }
    /// `Save(filename, ImageFormat)`
    pub fn save(&self, filename: Str, format: Gc<ImageFormat>) {
        if filename.is_null() {
            throw("System.ArgumentNullException", "filename");
        }
        // SAFETY: a live format object
        let kind = unsafe { (*format.ptr()).kind };
        if kind != 0 {
            return host::unimplemented("System.Drawing.Bitmap.Save(string, ImageFormat) with a format other than Png (the game only passes ImageFormat.Png)");
        }
        let bytes = crate::image::encode_png_bgra_premultiplied(self.width as u32, self.height as u32, &self.bgra);
        crate::sys::io::file_write_all_bytes(filename, Arr::from_slice(&bytes));
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.bgra = Vec::new();
    }
}

/// `Marshal.Copy(byte[] source, int startIndex, IntPtr destination, int length)`
pub fn marshal_copy_to_ptr(source: Arr<u8>, start: i32, dest: isize, length: i32) {
    if source.is_null() {
        throw("System.ArgumentNullException", "source");
    }
    if dest == 0 {
        throw("System.ArgumentNullException", "destination");
    }
    if start < 0 || length < 0 || start as i64 + length as i64 > source.len() as i64 {
        throw("System.ArgumentOutOfRangeException", "startIndex");
    }
    // SAFETY: the destination is a pointer the game got from `BitmapData.Scan0` for a bitmap at least
    // `length` bytes long (the same contract as the original, which also trusts the caller)
    unsafe { core::ptr::copy_nonoverlapping(source.as_slice().as_ptr().add(start as usize), dest as *mut u8, length as usize) };
}
/// `Marshal.Copy(IntPtr source, byte[] destination, int startIndex, int length)`
pub fn marshal_copy_from_ptr(source: isize, dest: Arr<u8>, start: i32, length: i32) {
    if dest.is_null() {
        throw("System.ArgumentNullException", "destination");
    }
    if start < 0 || length < 0 || start as i64 + length as i64 > dest.len() as i64 {
        throw("System.ArgumentOutOfRangeException", "startIndex");
    }
    // SAFETY: as above
    unsafe { core::ptr::copy_nonoverlapping(source as *const u8, dest.as_slice().as_ptr().add(start as usize) as *mut u8, length as usize) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_camera_path_writes_a_png_below_the_save_root() {
        // CaptureCamera.SaveImage(data, w, h, ImageFormat.Png, filename), non-FNA branch, with RGBA -> BGRA swapped data
        let (w, h) = (3, 2);
        let rgba: Vec<u8> = vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255];
        // premultiplied, as the render target holds it: the half-transparent green is (0, 128, 0, 128)
        let mut pre = rgba.clone();
        pre[4..8].copy_from_slice(&[0, 128, 0, 128]);
        let bgra: Vec<u8> = pre.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        let bmp = Bitmap::new_gc(w, h);
        let path = crate::scratch_dir().join("terraria-host-test-bitmap.png");
        let _ = std::fs::remove_file(&path);
        // the save root rule: a path below the install folder is refused, anywhere else is allowed
        unsafe {
            let b = &mut *bmp.ptr();
            let data = b.lock_bits(Rectangle::new(0, 0, w, h), 2, 0xE200B);
            marshal_copy_to_ptr(Arr::from_slice(&bgra), 0, (*data.ptr()).scan0, (w * h * 4) as i32);
            b.unlock_bits(data);
            b.save(Str::new(path.to_str().unwrap()), ImageFormat::get(0));
            assert!(rt::run(|| b.lock_bits(Rectangle::new(0, 0, 4, 4), 2, 0xE200B)).is_err(), "rectangle outside the bitmap");
        }
        let bytes = std::fs::read(&path).expect("png written");
        let (pw, ph, px) = crate::content::decode_png(&bytes).unwrap();
        assert_eq!((pw, ph), (3, 2));
        // straight alpha: (0, 128, 0, 128) premultiplied is (0, 255, 0, 128)
        assert_eq!(&px[0..4], &[255, 0, 0, 255]);
        assert_eq!(&px[4..8], &[0, 255, 0, 128]);
        assert_eq!(&px[8..12], &[0, 0, 0, 0]);
        assert_eq!(&px[12..16], &[10, 20, 30, 255]);
        let _ = std::fs::remove_file(&path);
        // other formats are refused with a reason, not written
        unsafe {
            (*bmp.ptr()).save(Str::new(path.to_str().unwrap()), ImageFormat::get(1));
        }
        assert!(!path.exists());
    }
}
