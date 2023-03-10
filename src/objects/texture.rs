use std::ptr::copy_nonoverlapping as memcpy;

use ash::vk;
use image::io::Reader;

use crate::*;

#[derive(Clone, Debug)]
pub struct Texture {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub layout: vk::ImageLayout,
    pub memory: vk::DeviceMemory,
    pub sampler: Option<vk::Sampler>,

    // TODO: maybe move most options on Texture2D struct?
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub layer_count: u32,

    staging_buffer: Buffer
}

impl Texture {
    pub unsafe fn destroy(&self, base: &Base) {
        base.device.destroy_image_view(self.view, None);
        base.device.destroy_image(self.image, None);

        if self.sampler.is_some() {
            base.device.destroy_sampler(self.sampler.unwrap(), None);
        }

        base.device.free_memory(self.memory, None);

        self.staging_buffer.destroy(base);
    }
}

// TODO: replace memcpy with Align
#[derive(Clone, Debug)]
pub struct Texture2D {
    pub data: Texture // TODO: find a better name
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

        let staging_buffer = Buffer::new(
            base,
            (std::mem::size_of::<u8>() * image_data.len()) as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            true
        );
        memcpy(image_data.as_ptr(), staging_buffer.ptr.unwrap().cast(), image_data.len());
        staging_buffer.unmap_memory(base);

        // TODO: here I should generate mipmaps

        // Create texture image and buffer
        let texture_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM) // TODO: maybe maybe expose it as parameter
            .extent(image_extent.into())
            .mip_levels(1) // TODO: implement mipmapping
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1) // TODO: implement multisampling
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build();

        let texture_image = base.device.create_image(&texture_create_info, None)?;
        let texture_memory_req = base.device.get_image_memory_requirements(texture_image);
        let texture_memory_index = find_memory_type_index(
            &texture_memory_req,
            &base.device_memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL
        ).unwrap();

        let texture_allocate_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(texture_memory_req.size)
            .memory_type_index(texture_memory_index)
            .build();

        let texture_memory = base.device.allocate_memory(&texture_allocate_info, None)?;
        base.device.bind_image_memory(texture_image, texture_memory, 0)?;

        record_submit_commandbuffer(
            &base.device,
            base.setup_command_buffer,
            base.setup_commands_reuse_fence,
            base.present_queue,
            &[], &[], &[],
            |device, texture_command_buffer| {
                let texture_subres_range = vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1)
                    .build();

                let texture_barrier = vk::ImageMemoryBarrier::builder()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .image(texture_image)
                    .subresource_range(texture_subres_range)
                    .build();

                // add barrier to command
                device.cmd_pipeline_barrier(
                    texture_command_buffer,
                    vk::PipelineStageFlags::ALL_COMMANDS, // BOTTOM_OF_PIPE
                    vk::PipelineStageFlags::ALL_COMMANDS, // TRANSFER
                    vk::DependencyFlags::empty(),
                    &[], &[], &[texture_barrier]
                );

                let buffer_copy_region = vk::BufferImageCopy::builder()
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
                    staging_buffer.buffer,
                    texture_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[buffer_copy_region]
                );

                let texture_barrier_end = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(texture_image)
                    .subresource_range(texture_subres_range)
                    .build();

                device.cmd_pipeline_barrier(
                    texture_command_buffer,
                    vk::PipelineStageFlags::ALL_COMMANDS, // TRANSFER
                    vk::PipelineStageFlags::ALL_COMMANDS, // FRAGMENT_SHADER
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

        Ok(Texture2D {
            data: Texture {
                image: texture_image,
                view: tex_image_view,
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                memory: texture_memory,

                width, height,
                mip_levels,
                layer_count: 1,

                sampler: Some(sampler),
                
                staging_buffer
            }
        })
    }
}