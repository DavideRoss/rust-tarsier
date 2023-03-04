use std::hash::{Hash, Hasher};

use ash::vk;
use nalgebra_glm as glm;
use crate::offset_of;

#[derive(Clone, Debug, Copy)]
pub struct Vertex {
    pub pos: glm::Vec3,
    pub uv: glm::Vec2
}

impl Vertex {
    pub fn new(pos: glm::Vec3, uv: glm::Vec2) -> Self {
        Vertex { pos, uv }
    }
    
    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX
        }
    }

    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Vertex, pos) as u32
            },

            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, uv) as u32,
            }
        ]
    }
}

impl PartialEq for Vertex {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos && self.uv == other.uv
    }
}

impl Eq for Vertex {}

impl Hash for Vertex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pos[0].to_bits().hash(state);
        self.pos[1].to_bits().hash(state);
        self.pos[2].to_bits().hash(state);
        self.uv[0].to_bits().hash(state);
        self.uv[1].to_bits().hash(state);
    }
}