//! The sprite queue behind `SpriteBatch`: XNA's vertex arithmetic, sort modes and texture runs, on plain values.
//!
//! Source: reference/xna/SpriteBatch.cs (`InternalDraw`, `PlatformRenderBatch`, `SortSprites`, `Flush`).

use terraria_core::xna::{Color, Matrix, Rectangle, Vector2};
use terraria_gen_rt::rt;

use super::draw_list::{DrawList, RawVertex, TextureId, Vertex};

/// `SpriteSortMode` (reference/xna/SpriteSortMode.cs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum SpriteSortMode {
    #[default]
    Deferred = 0,
    Immediate = 1,
    Texture = 2,
    BackToFront = 3,
    FrontToBack = 4,
}
impl SpriteSortMode {
    pub fn from_i32(v: i32) -> Option<SpriteSortMode> {
        use SpriteSortMode::*;
        [Deferred, Immediate, Texture, BackToFront, FrontToBack].get(usize::try_from(v).ok()?).copied()
    }
}

/// `SpriteEffects` flags.
pub const FLIP_HORIZONTALLY: i32 = 1;
pub const FLIP_VERTICALLY: i32 = 2;

/// Which two triangles a quad's four vertices form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuadLayout {
    /// vertices TL, TR, BR, BL; triangles (0,1,2) (0,2,3): XNA's sprite index buffer
    Xna,
    /// vertices TL, TR, BL, BR; triangles (0,1,2) (2,1,3): the app prototype's hand-ported batch
    Strip,
}

/// One queued sprite: four vertices in pixel space (before the batch transform).
#[derive(Clone, Copy, Debug)]
pub struct QueuedSprite {
    pub texture: TextureId,
    pub pos: [Vector2; 4],
    pub uv: [Vector2; 4],
    pub color: Color,
    pub depth: f32,
    pub layout: QuadLayout,
}

/// `SpriteBatch.InternalDraw` + the per-sprite part of `PlatformRenderBatch`: the four vertices XNA writes.
/// `destination` is (x, y, w, h); with `scale_destination` w and h are scale factors applied to the source size.
#[allow(clippy::too_many_arguments)]
pub fn xna_sprite(
    texture: TextureId,
    tex_size: (i32, i32),
    destination: [f32; 4],
    scale_destination: bool,
    source: Option<Rectangle>,
    color: Color,
    rotation: f32,
    origin: Vector2,
    effects: i32,
    depth: f32,
) -> QueuedSprite {
    let (mut dw, mut dh) = (destination[2], destination[3]);
    let src: [f32; 4] = match source {
        Some(r) => [r.x as f32, r.y as f32, r.width as f32, r.height as f32],
        None => [0.0, 0.0, tex_size.0 as f32, tex_size.1 as f32],
    };
    if scale_destination {
        dw *= src[2];
        dh *= src[3];
    }
    let inv_w = 1.0 / tex_size.0 as f32;
    let inv_h = 1.0 / tex_size.1 as f32;
    let (cos, sin) = if rotation != 0.0 { ((rotation as f64).cos() as f32, (rotation as f64).sin() as f32) } else { (1.0, 0.0) };
    let ox = if src[2] != 0.0 { origin.x / src[2] } else { origin.x * 2e32 };
    let oy = if src[3] != 0.0 { origin.y / src[3] } else { origin.y * 2e32 };
    const XC: [f32; 4] = [0.0, 1.0, 1.0, 0.0];
    const YC: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
    let mut pos = [Vector2::ZERO; 4];
    let mut uv = [Vector2::ZERO; 4];
    for j in 0..4 {
        let (mut cx, mut cy) = (XC[j], YC[j]);
        let px = (cx - ox) * dw;
        let py = (cy - oy) * dh;
        pos[j] = Vector2::new(destination[0] + px * cos - py * sin, destination[1] + px * sin + py * cos);
        if effects & FLIP_HORIZONTALLY != 0 {
            cx = 1.0 - cx;
        }
        if effects & FLIP_VERTICALLY != 0 {
            cy = 1.0 - cy;
        }
        uv[j] = Vector2::new((src[0] + cx * src[2]) * inv_w, (src[1] + cy * src[3]) * inv_h);
    }
    QueuedSprite { texture, pos, uv, color, depth, layout: QuadLayout::Xna }
}

fn three_way<T: PartialOrd>(a: T, b: T, greater: i32) -> i32 {
    if a > b {
        greater
    } else if a < b {
        -greater
    } else {
        0
    }
}

/// The queue of one `Begin`/`End` pair.
#[derive(Debug, Default)]
pub struct SpriteBatcher {
    pub sort_mode: SpriteSortMode,
    queue: Vec<QueuedSprite>,
}

impl SpriteBatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn push(&mut self, s: QueuedSprite) {
        self.queue.push(s);
    }

    pub fn queue(&self) -> &[QueuedSprite] {
        &self.queue
    }

    /// The order `Flush` renders the queue in (indices into the queue).
    ///
    /// `Texture`, `BackToFront` and `FrontToBack` sort an index array with `Array.Sort(int[], 0, n, comparer)`:
    /// .NET Framework 4.0's quicksort, which is NOT stable, so equal depths do not keep submission order; the
    /// exact algorithm is `rt::sys::depth_limited_quick_sort`.
    /// `Texture` compares the native texture pointers (higher address first, reference/xna/Texture.cs:364), which
    /// has no meaning here: textures are ordered by descending id instead. Which texture comes first was never
    /// defined for a game; that sprites of one texture become contiguous is what the mode is for.
    pub fn order(&self) -> Vec<u32> {
        let mut idx: Vec<u32> = (0..self.queue.len() as u32).collect();
        let q = &self.queue;
        match self.sort_mode {
            SpriteSortMode::Deferred | SpriteSortMode::Immediate => {}
            SpriteSortMode::Texture => rt::sys::depth_limited_quick_sort(&mut idx, &mut |x, y| three_way(q[x as usize].texture, q[y as usize].texture, -1)),
            SpriteSortMode::BackToFront => rt::sys::depth_limited_quick_sort(&mut idx, &mut |x, y| three_way(q[x as usize].depth, q[y as usize].depth, -1)),
            SpriteSortMode::FrontToBack => rt::sys::depth_limited_quick_sort(&mut idx, &mut |x, y| three_way(q[x as usize].depth, q[y as usize].depth, 1)),
        }
        idx
    }

    /// `Flush`: writes the queue's vertices (transformed by `transform`, mapped to the list's viewport) and calls
    /// `run(list, texture, first_vertex, vertex_count)` once per run of equal texture, in render order.
    pub fn flush(&mut self, list: &mut DrawList, transform: &Matrix, run: impl FnMut(&mut DrawList, TextureId, u32, u32)) {
        self.flush_as(list, Some(transform), run)
    }

    /// `Flush` under an effect pass that has its own vertex shader: writes the vertices XNA hands to Direct3D
    /// (pixel position, z = layer depth, texture coordinate, colour) as raw vertices; the vertex shader places them.
    /// The runs are ranges of the list's raw vertices.
    pub fn flush_raw(&mut self, list: &mut DrawList, run: impl FnMut(&mut DrawList, TextureId, u32, u32)) {
        self.flush_as(list, None, run)
    }

    fn flush_as(&mut self, list: &mut DrawList, transform: Option<&Matrix>, mut run: impl FnMut(&mut DrawList, TextureId, u32, u32)) {
        if self.queue.is_empty() {
            return;
        }
        let count = |list: &DrawList| if transform.is_some() { list.vertex_count() } else { list.raw_vertex_count() };
        let order = self.order();
        let mut run_start = count(list);
        let mut run_texture = self.queue[order[0] as usize].texture;
        for i in order {
            let s = &self.queue[i as usize];
            if s.texture != run_texture {
                let n = count(list) - run_start;
                run(list, run_texture, run_start, n);
                run_texture = s.texture;
                run_start = count(list);
            }
            let color = s.color.to_rgba_bytes();
            let Some(transform) = transform else {
                let v: [RawVertex; 4] = std::array::from_fn(|k| RawVertex { pos: [s.pos[k].x, s.pos[k].y, s.depth, 1.0], tex: [s.uv[k].x, s.uv[k].y, 0.0, 1.0], color });
                match s.layout {
                    QuadLayout::Xna => list.push_raw_vertices(&[v[0], v[1], v[2], v[0], v[2], v[3]]),
                    QuadLayout::Strip => list.push_raw_vertices(&[v[0], v[1], v[2], v[2], v[1], v[3]]),
                }
                continue;
            };
            let mut v = [Vertex { pos: [0.0; 2], uv: [0.0; 2], color }; 4];
            for k in 0..4 {
                let p = match s.layout {
                    // the prototype's arithmetic, kept bit for bit
                    QuadLayout::Strip => Vector2::transform(s.pos[k], transform),
                    QuadLayout::Xna => transform_xyz(s.pos[k], s.depth, transform),
                };
                v[k].pos = list.to_clip(p);
                v[k].uv = [s.uv[k].x, s.uv[k].y];
            }
            match s.layout {
                QuadLayout::Xna => list.push_vertices(&[v[0], v[1], v[2], v[0], v[2], v[3]]),
                QuadLayout::Strip => list.push_vertices(&[v[0], v[1], v[2], v[2], v[1], v[3]]),
            }
        }
        let n = count(list) - run_start;
        run(list, run_texture, run_start, n);
        self.queue.clear();
    }
}

/// `(x, y, z, 1) * m`, projected: what the sprite vertex shader does with `MatrixTransform` before the viewport
/// mapping (which `DrawList::to_clip` applies).
pub fn transform_xyz(p: Vector2, z: f32, m: &Matrix) -> Vector2 {
    let x = p.x * m.m11 + p.y * m.m21 + z * m.m31 + m.m41;
    let y = p.x * m.m12 + p.y * m.m22 + z * m.m32 + m.m42;
    let w = p.x * m.m14 + p.y * m.m24 + z * m.m34 + m.m44;
    if w == 1.0 {
        Vector2::new(x, y)
    } else {
        Vector2::new(x / w, y / w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(tex: u32, depth: f32) -> QueuedSprite {
        xna_sprite(TextureId(tex), (8, 8), [0.0, 0.0, 1.0, 1.0], true, None, Color::WHITE, 0.0, Vector2::ZERO, 0, depth)
    }

    #[test]
    fn vertices_match_xna_arithmetic() {
        // Draw(tex 64x32, position (100,50), source (8,4,16,8), rotation 0, origin (8,4), scale (2,3), FlipHorizontally)
        let s = xna_sprite(TextureId(1), (64, 32), [100.0, 50.0, 2.0, 3.0], true, Some(Rectangle::new(8, 4, 16, 8)), Color::WHITE, 0.0, Vector2::new(8.0, 4.0), FLIP_HORIZONTALLY, 0.5);
        // destination size 32x24, origin fraction (0.5, 0.5): corners at +-16, +-12 around the position
        assert_eq!(s.pos.map(|p| (p.x, p.y)), [(84.0, 38.0), (116.0, 38.0), (116.0, 62.0), (84.0, 62.0)]);
        // u flipped: left vertices sample the source's right edge (8+16)/64, right ones 8/64
        assert_eq!(s.uv.map(|p| (p.x, p.y)), [(0.375, 0.125), (0.125, 0.125), (0.125, 0.375), (0.375, 0.375)]);
        // destination rectangle overload: origin is in source texels, scaled by dest/source
        let r = xna_sprite(TextureId(1), (64, 32), [10.0, 10.0, 32.0, 16.0], false, Some(Rectangle::new(0, 0, 16, 8)), Color::WHITE, 0.0, Vector2::new(16.0, 8.0), 0, 0.0);
        assert_eq!((r.pos[0].x, r.pos[0].y, r.pos[2].x, r.pos[2].y), (-22.0, -6.0, 10.0, 10.0));
        // rotation by 90 degrees uses (float)Math.Cos / Sin of the double
        let q = xna_sprite(TextureId(1), (2, 2), [0.0, 0.0, 1.0, 1.0], true, None, Color::WHITE, std::f32::consts::FRAC_PI_2, Vector2::ZERO, 0, 0.0);
        let c = (std::f32::consts::FRAC_PI_2 as f64).cos() as f32;
        assert_eq!((q.pos[1].x, q.pos[1].y), (2.0 * c, 2.0));
    }

    #[test]
    fn sort_modes() {
        let mut b = SpriteBatcher::new();
        for (t, d) in [(3, 0.5), (1, 0.9), (3, 0.1), (2, 0.5), (1, 0.0)] {
            b.push(sprite(t, d));
        }
        b.sort_mode = SpriteSortMode::Deferred;
        assert_eq!(b.order(), [0, 1, 2, 3, 4]);
        b.sort_mode = SpriteSortMode::BackToFront;
        let depths: Vec<f32> = b.order().iter().map(|i| b.queue[*i as usize].depth).collect();
        assert_eq!(depths, [0.9, 0.5, 0.5, 0.1, 0.0]);
        b.sort_mode = SpriteSortMode::FrontToBack;
        let depths: Vec<f32> = b.order().iter().map(|i| b.queue[*i as usize].depth).collect();
        assert_eq!(depths, [0.0, 0.1, 0.5, 0.5, 0.9]);
        b.sort_mode = SpriteSortMode::Texture;
        let texs: Vec<u32> = b.order().iter().map(|i| b.queue[*i as usize].texture.0).collect();
        assert_eq!(texs, [3, 3, 2, 1, 1]);
    }

    /// Equal keys: the order is the one `Array.Sort` leaves, not submission order. The expected index order was
    /// produced by real .NET (tests/dotnet/sprite_sort.ps1 writes sprite_sort.expected).
    #[test]
    fn sort_is_not_stable_and_matches_dotnet() {
        let mut b = SpriteBatcher::new();
        for d in [0.5, 0.5, 0.2, 0.5, 0.9, 0.5, 0.2, 0.5, 0.5, 0.9, 0.5, 0.5, 0.2, 0.9, 0.5, 0.5, 0.5, 0.2, 0.5, 0.9] {
            b.push(sprite(1, d));
        }
        b.sort_mode = SpriteSortMode::BackToFront;
        let expected: [u32; 20] = include!("../../tests/dotnet/sprite_sort.expected");
        assert_eq!(b.order(), expected);
    }

    #[test]
    fn flush_makes_one_run_per_texture_change() {
        let mut l = DrawList::new();
        l.set_back_buffer_size(100, 100);
        let mut b = SpriteBatcher::new();
        for t in [1, 1, 2, 1] {
            b.push(sprite(t, 0.0));
        }
        let mut runs = Vec::new();
        b.flush(&mut l, &Matrix::IDENTITY, |_, t, first, n| runs.push((t.0, first, n)));
        assert_eq!(runs, [(1, 0, 12), (2, 12, 6), (1, 18, 6)]);
        assert!(b.is_empty());
        // 8x8 sprite at the origin of a 100x100 viewport: top-left is clip (-1, 1)
        assert_eq!(l.vertices()[0].pos, [-1.0, 1.0]);
        assert_eq!(l.vertices()[2].pos, [8.0 / 100.0 * 2.0 - 1.0, 1.0 - 8.0 / 100.0 * 2.0]);
    }
}
