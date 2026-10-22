//! `ContentManager` for the player's own install: loads `Content/<name>.xnb` on demand, caches by name.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::Resource;
use terraria_formats::xnb::{self, Asset as XnbAsset};

use crate::gfx::{DrawList, Texture2D};

#[derive(Resource)]
pub struct Content {
    /// `<install>/Content`
    pub root: PathBuf,
    textures: HashMap<String, Texture2D>,
}

impl Content {
    /// Install folder: `TERRARIA_DIR`, else the path this project was built against.
    pub fn locate() -> Result<Self, String> {
        let dir = std::env::var_os("TERRARIA_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Users\user\Documents\Games\Terraria"));
        let root = dir.join("Content");
        if !root.is_dir() {
            return Err(format!("Terraria content folder not found at {} (set TERRARIA_DIR to the install folder)", root.display()));
        }
        Ok(Self { root, textures: HashMap::new() })
    }

    /// `Main.Assets.Request<Texture2D>(name)`, e.g. `"Images/Tiles_0"`.
    pub fn texture(&mut self, list: &mut DrawList, name: &str) -> Result<Texture2D, String> {
        if let Some(t) = self.textures.get(name) {
            return Ok(t.clone());
        }
        let path = self.root.join(format!("{name}.xnb"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = xnb::read_xnb(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))?;
        let XnbAsset::Texture2D(tex) = file.asset else {
            return Err(format!("{}: not a Texture2D ({})", path.display(), file.asset.kind()));
        };
        let rgba = tex.top_mip_rgba8().map_err(|e| format!("{}: {e:?}", path.display()))?;
        let id = list.create_texture(tex.width, tex.height, Some(rgba), false);
        let t = Texture2D { id, width: tex.width as i32, height: tex.height as i32 };
        self.textures.insert(name.to_owned(), t.clone());
        Ok(t)
    }
}
