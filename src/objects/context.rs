use ash::vk;

pub struct VkContext {
    entry: Entry,
    instance: Instance,
    debug_report_callback: Option<(DebugUtils, vk::DebugUtilsMessengerEXT)>,
    surface: Surface,
    surface_khr: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Device
}

impl VkContext {
    pub fn instance(&self) -> &Instance { &self.instance }
    pub fn surface(&self) -> &Surface { &self.surface }
    pub fn surface_khr(&self) -> vk::SurfaceKHR { self.surface_khr }
    pub fn physical_device(&self) -> vk::PhysicalDevice { self.PhysicalDevice }
    pub fn device(&self) -> &Device { &self.device }
}

// https://github.com/adrien-ben/vulkan-tutorial-rs/blob/master/src/context.rs