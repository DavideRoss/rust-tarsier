use nalgebra_glm as glm;

#[derive(Copy, Clone, Debug)]
pub struct UniformBufferObject {
    pub model: glm::Mat4,
    pub view: glm::Mat4,
    pub projection: glm::Mat4,
}
