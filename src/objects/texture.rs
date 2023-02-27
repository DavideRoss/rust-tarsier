use ash::{vk, Device, util::Align};
use image::{io::Reader, GenericImageView};

use crate::*;

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
        base: &Base,
        filename: &str
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Read image and get info and data
        let image = Reader::open(filename)?.decode()?.to_rgba8();
        let (width, height) = image.dimensions();
        let image_extent = vk::Extent2D { width, height };
        let mip_levels = (width.max(height) as f32).log2().floor() as u32 + 1;
        let image_data = image.into_raw();

        // Create image buffer
        let staging_buffer_info = vk::BufferCreateInfo {
            size: (std::mem::size_of::<u8>() * image_data.len()) as u64,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let staging_buffer = base.device.create_buffer(&staging_buffer_info, None).unwrap();

        // Copy image into buffer
        let staging_buffer_memory_req = base.device.get_buffer_memory_requirements(staging_buffer);
        let staging_buffer_memory_index = find_memory_type_index(
            &staging_buffer_memory_req, 
            &base.device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        ).expect("Unable ot find suitable memory type for image buffer");

        // TODO: change with builder pattern
        let staging_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: staging_buffer_memory_req.size,
            memory_type_index: staging_buffer_memory_index,
            ..Default::default()
        };

        let staging_buffer_memory = base.device.allocate_memory(
            &staging_buffer_allocate_info,
            None
        ).unwrap();
        let image_ptr = base.device.map_memory(
            staging_buffer_memory, 
            0,
            staging_buffer_memory_req.size,
            vk::MemoryMapFlags::empty()
        ).unwrap();
        let mut image_slice = Align::new(
            image_ptr,
            std::mem::align_of::<u8>() as u64,
            staging_buffer_memory_req.size
        );
        image_slice.copy_from_slice(&image_data);
        base.device.unmap_memory(staging_buffer_memory);
        base.device.bind_buffer_memory(staging_buffer, staging_buffer_memory, 0).unwrap();

        // Create texture image and buffer
        let texture_create_info = vk::ImageCreateInfo {
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::R8G8B8A8_UNORM,
            extent: image_extent.into(),
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let texture_image = base.device.create_image(&texture_create_info, None).unwrap();
        let texture_memory_req = base.device.get_image_memory_requirements(texture_image);
        let texture_memory_index = find_memory_type_index(
            &texture_memory_req,
            &base.device_memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL
        ).expect("Unable to find suitable memory index for depth image");

        let texture_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: texture_memory_req.size,
            memory_type_index: texture_memory_index,
            ..Default::default()
        };
        let texture_memory = base.device.allocate_memory(&texture_allocate_info, None).unwrap();
        base.device.bind_image_memory(texture_image, texture_memory, 0).expect("Unable to bind depth image memory");

        record_submit_commandbuffer(
            &base.device,
            base.setup_command_buffer,
            base.setup_commands_reuse_fence,
            base.present_queue,
            &[], &[], &[],
            |device, texture_command_buffer| {
                // Create texture barrier
                let texture_barrier = vk::ImageMemoryBarrier {
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    image: texture_image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                // add barrier to command
                device.cmd_pipeline_barrier(
                    texture_command_buffer,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[], &[], &[texture_barrier]
                );

                let buffer_copy_regions = vk::BufferImageCopy::builder()
                    .image_subresource(
                        vk::ImageSubresourceLayers::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1)
                            .build()
                    )
                    .image_extent(image_extent.into())
                    .build();

                device.cmd_copy_buffer_to_image(
                    texture_command_buffer,
                    staging_buffer,
                    texture_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[buffer_copy_regions]
                );

                let texture_barrier_end = vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::SHADER_READ,
                    old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image: texture_image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                device.cmd_pipeline_barrier(
                    texture_command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(), 
                    &[], &[], &[texture_barrier_end]
                );
            }
        );

        let sampler_info = vk::SamplerCreateInfo {
            mag_filter: vk::Filter::LINEAR,
            min_filter: vk::Filter::LINEAR,
            mipmap_mode: vk::SamplerMipmapMode::LINEAR,
            address_mode_u: vk::SamplerAddressMode::MIRRORED_REPEAT,
            address_mode_v: vk::SamplerAddressMode::MIRRORED_REPEAT,
            address_mode_w: vk::SamplerAddressMode::MIRRORED_REPEAT,
            max_anisotropy: 1.0,
            border_color: vk::BorderColor::FLOAT_OPAQUE_WHITE,
            compare_op: vk::CompareOp::NEVER,
            ..Default::default()
        };

        let sampler = base.device.create_sampler(&sampler_info, None).unwrap();

        let tex_image_view_info = vk::ImageViewCreateInfo {
            view_type: vk::ImageViewType::TYPE_2D,
            format: texture_create_info.format,
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A
            },
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                level_count: 1,
                layer_count: 1,
                ..Default::default()
            },
            image: texture_image,
            ..Default::default()
        };

        let tex_image_view = base.device.create_image_view(&tex_image_view_info, None).unwrap();

        let descriptor = vk::DescriptorImageInfo { 
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image_view: tex_image_view,
            sampler
        };

        // TODO: where the fuck should I destroy them?
        // base.device.destroy_buffer(staging_buffer, None);
        // base.device.free_memory(staging_buffer_memory, None);

        Ok(Texture2D {
            texture: Texture {
                image: texture_image,
                image_view: tex_image_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_memory: texture_memory,

                width, height,
                mip_levels,
                layer_count: 1,

                descriptor,
                sampler: Some(sampler)
            }
        })
    }
}