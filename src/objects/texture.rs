use ash::{vk, Device};

use std::io::Cursor;
use image::{io::Reader, GenericImageView};

#[derive(Clone, Debug)]
pub struct Texture {
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub image_layout: vk::ImageLayout,
    pub image_memory: vk::DeviceMemory,

    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub layer_count: u32,

    pub descriptor: vk::DescriptorImageInfo,
    pub sampler: Option<vk::Sampler>
}

impl Texture {
    fn update_descriptor(&mut self) {
        self.descriptor.sampler = self.sampler.unwrap_or(vk::Sampler::null());
        self.descriptor.image_view = self.image_view;
        self.descriptor.image_layout = self.image_layout;
    }
}

#[derive(Clone, Debug)]
pub struct Texture2D {
    pub texture: Texture
}

impl Texture2D {
    pub unsafe fn load_from_file(
        device: &Device,
        filename: &str
    ) -> Result<Self> {
        let image = Reader::open(filename)?.decode()?;
        let (width, height) = image.dimensions();
        let mip_levels = (width.max(height) as f32).log2().floor() as u32 + 1;

        Ok(())
    }
}