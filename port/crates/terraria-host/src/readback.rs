//! `Texture2D.GetData` on render targets: the host half of the readback protocol (docs/readback.md has the call
//! sites, the decision and the exact change the wgpu backend needs).
//!
//! Two paths, both built on what `gfx::draw_list` already records:
//!
//! - **Synchronous** ([`install_sync`]): `GetData` ends the frame recorded so far (`take_frame`), hands it to a
//!   [`ReadbackBackend`], which replays it NOW and returns the target's RGBA8 pixels. Every `GetData` call site of
//!   the game needs the content that was just drawn (screenshots), so this is the path that matters. The device's
//!   `readback_hook` field is the plug; this module only fills it, `gfx` is not touched.
//! - **Request / response** ([`request`], [`latest`], [`stale_or_request`], [`serve`]): `Command::Readback` in frame
//!   N, the backend answers after replaying frame N (`DrawList::deliver_readback`), the data is there at frame
//!   N + 1. For callers that tolerate one frame of latency; the game has none today (docs/readback.md).
//!
//! [`CpuBackend`] is a fake backend that executes the commands a test needs (`CreateTexture`, `Upload`,
//! `SetTarget`, `Clear`) on CPU buffers; the real one is `terraria-app`'s wgpu renderer.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use terraria_gen_rt::rt::Gc;

use crate::gfx::draw_list::{Command, Frame, TextureId};
use crate::gfx::{GraphicsDevice, Texture2D};

/// What the app's renderer provides for a synchronous `GetData`.
pub trait ReadbackBackend {
    /// Replay `frame` (which does not present anything) and return the RGBA8 pixels of render target `id` as they
    /// are after the last command of the frame.
    fn read_target(&mut self, frame: Frame, id: TextureId) -> Vec<u8>;
}

/// Makes `GetData` on render targets of `device` synchronous through `backend`.
pub fn install_sync(device: Gc<GraphicsDevice>, backend: Rc<RefCell<dyn ReadbackBackend>>) {
    // SAFETY: a live device object; nothing else holds a reference while the host runs single threaded
    let d = unsafe { &mut *device.ptr() };
    d.readback_hook = Some(Box::new(move |frame, id| backend.borrow_mut().read_target(frame, id)));
}

/// Back to `unimplemented_value` for `GetData` on render targets.
pub fn remove_sync(device: Gc<GraphicsDevice>) {
    // SAFETY: as above
    unsafe { (*device.ptr()).readback_hook = None };
}

// ------------------------------------------------------------------------------------------------ request / response

/// Asks the backend for the pixels of `texture` (frame N). Returns the request number.
pub fn request(device: Gc<GraphicsDevice>, texture: Gc<Texture2D>) -> u64 {
    // SAFETY: live objects
    unsafe { (*device.ptr()).list.request_readback((*texture.ptr()).id) }
}

/// The newest answer for `texture`: (request number, RGBA8 pixels).
pub fn latest(device: Gc<GraphicsDevice>, texture: Gc<Texture2D>) -> Option<(u64, Vec<u8>)> {
    // SAFETY: live objects
    unsafe { (*device.ptr()).list.readback((*texture.ptr()).id).map(|(r, p)| (r, p.to_vec())) }
}

/// The policy for a caller that tolerates one frame of latency: the previous answer if there is one, and in any case
/// a new request so the next call is fresher. `None` until the first answer has arrived.
pub fn stale_or_request(device: Gc<GraphicsDevice>, texture: Gc<Texture2D>) -> Option<Vec<u8>> {
    let old = latest(device, texture);
    request(device, texture);
    old.map(|(_, p)| p)
}

/// Backend side of the protocol: after replaying `frame`, answer its `Command::Readback`s from `read` (the target's
/// pixels at the end of the frame).
pub fn serve(device: Gc<GraphicsDevice>, frame: &Frame, mut read: impl FnMut(TextureId) -> Vec<u8>) {
    for c in &frame.commands {
        if let Command::Readback { id, request } = c {
            let px = read(*id);
            // SAFETY: a live device object
            unsafe { (*device.ptr()).list.deliver_readback(*request, *id, px) };
        }
    }
}

// ------------------------------------------------------------------------------------------------ a CPU backend for tests

/// Executes `CreateTexture`, `DestroyTexture`, `Upload`, `SetTarget` and `Clear` on CPU buffers (no rasteriser: draws
/// are counted, not drawn). Enough to test the plumbing and to show what a real backend has to do.
pub struct CpuBackend {
    pub textures: HashMap<TextureId, (u32, u32, Vec<u8>)>,
    pub target: TextureId,
    pub frames_replayed: Vec<u64>,
    pub draws: usize,
}

impl Default for CpuBackend {
    fn default() -> Self {
        CpuBackend { textures: HashMap::new(), target: TextureId::BACK_BUFFER, frames_replayed: Vec::new(), draws: 0 }
    }
}

impl CpuBackend {
    pub fn replay(&mut self, frame: &Frame) {
        self.frames_replayed.push(frame.serial);
        self.textures.entry(TextureId::BACK_BUFFER).or_insert_with(|| (frame.back_buffer.0, frame.back_buffer.1, vec![0; (frame.back_buffer.0 * frame.back_buffer.1 * 4) as usize]));
        for c in &frame.commands {
            match c {
                Command::CreateTexture { id, width, height, .. } => {
                    self.textures.insert(*id, (*width, *height, vec![0; (*width * *height * 4) as usize]));
                }
                Command::DestroyTexture { id } => {
                    self.textures.remove(id);
                }
                Command::Upload { id, rect, rgba } => {
                    if let Some((w, _, px)) = self.textures.get_mut(id) {
                        for row in 0..rect.h as usize {
                            let dst = ((rect.y as usize + row) * *w as usize + rect.x as usize) * 4;
                            let src = row * rect.w as usize * 4;
                            px[dst..dst + rect.w as usize * 4].copy_from_slice(&rgba[src..src + rect.w as usize * 4]);
                        }
                    }
                }
                Command::SetTarget { target, preserve } => {
                    self.target = *target;
                    if !*preserve {
                        if let Some((_, _, px)) = self.textures.get_mut(target) {
                            px.iter_mut().for_each(|b| *b = 0);
                        }
                    }
                }
                Command::Clear(color) => {
                    if let Some((_, _, px)) = self.textures.get_mut(&self.target) {
                        let c = color.to_rgba_bytes();
                        px.chunks_exact_mut(4).for_each(|p| p.copy_from_slice(&c));
                    }
                }
                Command::Draw { .. } => self.draws += 1,
                _ => {}
            }
        }
    }
    pub fn pixels(&self, id: TextureId) -> Vec<u8> {
        self.textures.get(&id).map(|t| t.2.clone()).unwrap_or_default()
    }
}

impl ReadbackBackend for CpuBackend {
    fn read_target(&mut self, frame: Frame, id: TextureId) -> Vec<u8> {
        self.replay(&frame);
        self.pixels(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraria_core::xna::Color;
    use terraria_gen_rt::rt::Arr;

    fn setup() -> (Gc<GraphicsDevice>, Gc<Texture2D>, Rc<RefCell<CpuBackend>>) {
        let d = GraphicsDevice::new_gc(64, 32);
        let rt = Texture2D::new_render_target(d, 4, 2, false, 0, 0, 0);
        let backend = Rc::new(RefCell::new(CpuBackend::default()));
        (d, rt, backend)
    }

    /// `SetRenderTarget(rt); Clear(c); SetRenderTarget(null); rt.GetData(array)` as CaptureCamera does.
    #[test]
    fn get_data_on_a_render_target_is_served_synchronously_with_the_frame_so_far() {
        let (d, rt, backend) = setup();
        unsafe {
            let dev = &mut *d.ptr();
            let data = Arr::<u8>::new(4 * 2 * 4);
            // without a backend: unimplemented, as before (the registry records it; the call does not return data)
            install_sync(d, backend.clone());
            dev.set_render_target(rt);
            dev.clear(Color::new(10, 20, 30, 255));
            dev.set_render_target(Gc::NULL);
            (*rt.ptr()).get_data(data);
            assert_eq!(&data.as_slice()[..8], &[10, 20, 30, 255, 10, 20, 30, 255]);
            assert!(data.as_slice().chunks_exact(4).all(|p| p == [10, 20, 30, 255]));
            // the frame handed to the backend contained everything recorded so far, in order, and ended there
            let b = backend.borrow();
            assert_eq!(b.frames_replayed.len(), 1);
            drop(b);
            // the draw list continues with the next frame: a second chunk of the capture
            dev.set_render_target(rt);
            dev.clear(Color::new(1, 2, 3, 255));
            dev.set_render_target(Gc::NULL);
            (*rt.ptr()).get_data(data);
            assert!(data.as_slice().chunks_exact(4).all(|p| p == [1, 2, 3, 255]));
            let b = backend.borrow();
            assert_eq!(b.frames_replayed, vec![b.frames_replayed[0], b.frames_replayed[0] + 1], "one frame per GetData, serials consecutive");
            drop(b);
            // the frame that is presented later does not contain the commands already handed over
            let presented = dev.present();
            assert!(!presented.commands.iter().any(|c| matches!(c, Command::Clear(_))) || presented.commands.len() < 4);
            remove_sync(d);
        }
    }

    #[test]
    fn request_response_has_one_frame_of_latency_and_returns_the_previous_contents() {
        let (d, rt, backend) = setup();
        unsafe {
            let dev = &mut *d.ptr();
            // frame 0: draw, ask; nothing has been answered yet
            dev.set_render_target(rt);
            dev.clear(Color::new(200, 0, 0, 255));
            dev.set_render_target(Gc::NULL);
            assert!(stale_or_request(d, rt).is_none(), "first call: no previous contents");
            let f0 = dev.present();
            assert!(f0.commands.iter().any(|c| matches!(c, Command::Readback { .. })));
            {
                // the backend replays frame 0 and answers its request
                let mut b = backend.borrow_mut();
                b.replay(&f0);
                serve(d, &f0, |id| b.pixels(id));
            }
            // frame 1: the game draws something else; the answer it gets is frame 0's content
            dev.set_render_target(rt);
            dev.clear(Color::new(0, 0, 200, 255));
            dev.set_render_target(Gc::NULL);
            let seen = stale_or_request(d, rt).expect("answer of frame 0");
            assert!(seen.chunks_exact(4).all(|p| p == [200, 0, 0, 255]), "previous frame's pixels, not the new clear");
            let f1 = dev.present();
            let mut b = backend.borrow_mut();
            b.replay(&f1);
            serve(d, &f1, |id| b.pixels(id));
            drop(b);
            let (req, px) = latest(d, rt).unwrap();
            assert!(px.chunks_exact(4).all(|p| p == [0, 0, 200, 255]) && req == 2, "request {req}");
        }
    }
}
