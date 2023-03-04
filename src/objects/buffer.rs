use std::ffi::c_void;

use crate::{Base, find_memory_type_index};
use ash::vk;

#[derive(Clone, Debug)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub device_memory: vk::DeviceMemory,
    pub ptr: Option<*mut c_void>,
    pub size: u64,
}

impl Buffer {
    pub unsafe fn new(
        base: &Base,
        size: u64,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        memory_type_flags: vk::MemoryPropertyFlags,
        automap: bool
    ) -> Self {
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size)
            .usage(usage)
            .sharing_mode(sharing_mode)
            .build();

        let buffer = base.device.create_buffer(&buffer_info, None).unwrap();

        let buffer_mem_req = base.device.get_buffer_memory_requirements(buffer);
        let buffer_mem_index = find_memory_type_index(
            &buffer_mem_req,
            &base.device_memory_properties,
            memory_type_flags
        ).unwrap();

        let buffer_allocate_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(buffer_mem_req.size)
            .memory_type_index(buffer_mem_index)
            .build();

        let device_memory = base.device.allocate_memory(&buffer_allocate_info, None).unwrap();
        base.device.bind_buffer_memory(buffer, device_memory, 0).unwrap();

        let ptr = if automap {
            Some(base.device.map_memory(device_memory, 0, buffer_mem_req.size, vk::MemoryMapFlags::empty()).unwrap())
        } else {
            None
        };

        Buffer {
            buffer,
            device_memory,
            ptr,
            size: buffer_mem_req.size
        }
    }

    pub unsafe fn unmap_memory(&self, base: &Base) {
        base.device.unmap_memory(self.device_memory);
    }

    pub unsafe fn destroy(&self, base: &Base) {
        base.device.destroy_buffer(self.buffer, None);
        base.device.free_memory(self.device_memory, None);
    }
}