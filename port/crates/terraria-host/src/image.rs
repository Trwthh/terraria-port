//! PNG output for `Texture2D.SaveAsPng` and `System.Drawing.Bitmap.Save`, next to the PNG reader in `content.rs`
//! (`Texture2D.FromStream`, which is what `ReLogic.Content.Readers.PngReader` and the workshop preview use).
//!
//! The game calls neither `SaveAsPng` nor `SaveAsJpeg` (decompile grep): screenshots go through `System.Drawing.Bitmap`
//! (`sys/drawing.rs`). `SaveAsPng` is provided anyway for the full-size case; a resize (width / height different
//! from the texture's) and JPEG are `unimplemented` (nothing calls them, and an image resampler would be invented
//! behaviour). JPEG decoding is not provided either: `FromStream` is PNG only (README, "Content").

use terraria_gen_rt::host;
use terraria_gen_rt::rt::{io::Stream, Gc};

use crate::gfx::Texture2D;

/// RGBA8 rows -> PNG file bytes.
pub fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, width, height);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().and_then(|mut w| w.write_image_data(rgba)).expect("PNG encoding into memory cannot fail for a well-formed buffer");
    }
    out
}

/// B, G, R, A rows with premultiplied colour (`Format32bppPArgb`) -> PNG file bytes with straight alpha.
pub fn encode_png_bgra_premultiplied(width: u32, height: u32, bgra: &[u8]) -> Vec<u8> {
    let rgba: Vec<u8> = bgra
        .chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as u32;
            let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
            [un(p[2]), un(p[1]), un(p[0]), p[3]]
        })
        .collect();
    encode_png_rgba(width, height, &rgba)
}

/// `Texture2D.SaveAsPng(stream, width, height)`: the texture's pixels as stored (no un-premultiplying, as XNA).
pub fn texture_save_as_png(texture: Gc<Texture2D>, stream: Gc<Stream>, width: i32, height: i32) {
    // SAFETY: live objects
    let t = unsafe { &mut *texture.ptr() };
    if (width, height) != (t.width(), t.height()) {
        return host::unimplemented("Microsoft.Xna.Framework.Graphics.Texture2D.SaveAsPng(Stream, int, int) with a size different from the texture's (resampling)");
    }
    let Some(px) = t.pixels() else {
        return host::unimplemented("Microsoft.Xna.Framework.Graphics.Texture2D.SaveAsPng(Stream, int, int) on a render target without a readback backend");
    };
    let bytes = encode_png_rgba(width as u32, height as u32, &px);
    // SAFETY: a live stream
    unsafe { (*stream.ptr()).write_slice(&bytes) };
}

/// `Texture2D.SaveAsJpeg(stream, width, height)`
pub fn texture_save_as_jpeg(_texture: Gc<Texture2D>, _stream: Gc<Stream>, _width: i32, _height: i32) {
    host::unimplemented("Microsoft.Xna.Framework.Graphics.Texture2D.SaveAsJpeg(Stream, int, int): no JPEG encoder; the game never calls it");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::GraphicsDevice;

    #[test]
    fn png_round_trip_through_stream() {
        let d = GraphicsDevice::new_gc(8, 8);
        let px: Vec<u8> = (0..16).map(|i| (i * 16) as u8).collect();
        let t = Texture2D::from_rgba(d, 2, 2, px.clone(), true);
        let s = Stream::new_memory();
        texture_save_as_png(t, s, 2, 2);
        // SAFETY: live stream
        let bytes = unsafe { (*s.ptr()).bytes().to_vec() };
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
        let (w, h, back) = crate::content::decode_png(&bytes).unwrap();
        assert_eq!((w, h, back), (2, 2, px));
        // FromStream reads what SaveAsPng wrote
        let t2 = crate::content::texture_from_stream(d, Stream::from_vec(bytes));
        assert_eq!(unsafe { (*t2.ptr()).pixels().unwrap() }, (0..16).map(|i| (i * 16) as u8).collect::<Vec<u8>>());
        assert_eq!(encode_png_bgra_premultiplied(1, 1, &[0, 0, 0, 0]).len() > 8, true);
    }
}
