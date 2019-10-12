use std::path::*;
use rg3d_core::visitor::{Visit, VisitResult, Visitor};
use crate::renderer::gpu_texture::GpuTexture;
use image::GenericImageView;

pub struct Texture {
    pub(in crate) path: PathBuf,
    pub(in crate) width: u32,
    pub(in crate) height: u32,
    pub(in crate) gpu_tex: Option<GpuTexture>,
    pub(in crate) bytes: Vec<u8>,
    pub(in crate) kind: TextureKind
}

impl Default for Texture {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            width: 0,
            height: 0,
            gpu_tex: None,
            bytes: Vec::new(),
            kind: TextureKind::RGBA8
        }
    }
}

impl Visit for Texture {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        let mut kind = self.kind.id();
        kind.visit("KindId", visitor)?;
        if visitor.is_reading() {
            self.kind = TextureKind::new(kind)?;
        }

        self.path.visit("Path", visitor)?;

        visitor.leave_region()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TextureKind {
    R8,
    RGB8,
    RGBA8,
}

impl TextureKind {
    pub fn new(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(TextureKind::R8),
            1 => Ok(TextureKind::RGB8),
            2 => Ok(TextureKind::RGBA8),
            _ => Err(format!("Invalid texture kind {}!", id))
        }
    }

    pub fn id(&self) -> u32 {
        match self {
            TextureKind::R8 => 0,
            TextureKind::RGB8 => 1,
            TextureKind::RGBA8 => 2,
        }
    }
}

impl Texture {
    pub(in crate) fn load_from_file(path: &Path, kind: TextureKind) -> Result<Texture, image::ImageError> {
        let dyn_img = image::open(path)?;

        let width = dyn_img.width();
        let height = dyn_img.height();

        let bytes = match kind {
            TextureKind::R8 => dyn_img.to_luma().into_raw(),
            TextureKind::RGB8 => dyn_img.to_rgb().into_raw(),
            TextureKind::RGBA8 => dyn_img.to_rgba().into_raw(),
        };

        Ok(Texture {
            kind,
            width,
            height,
            bytes,
            path: PathBuf::from(path),
            gpu_tex: None,
        })
    }

    pub(in crate) fn bind(&self, sampler_index: usize) {
        if let Some(texture) = &self.gpu_tex {
            texture.bind(sampler_index)
        }
    }
}