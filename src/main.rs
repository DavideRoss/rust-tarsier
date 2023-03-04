mod objects;
use crate::objects::*;

use ash::vk;
use ash::util::*;

use std::default::Default;
use std::ffi::CStr;
use std::io::Cursor;
use std::mem::align_of;

use nalgebra_glm as glm;

// TODO: replace all struct declaration with builders

fn main() {
    unsafe {
        let base = Base::new(1920, 1080);

        // ================================================================
        // RENDERPASS
        // ================================================================

        let renderpass_attachments = [
            vk::AttachmentDescription {
                format: base.surface_format.format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },

            vk::AttachmentDescription {
                format: vk::Format::D16_UNORM,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            }
        ];

        let color_attachment_refs = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        }];

        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
        };

        let dependencies = [vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ..Default::default()
        }];

        let subpass = vk::SubpassDescription::builder()
            .color_attachments(&color_attachment_refs)
            .depth_stencil_attachment(&depth_attachment_ref)
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .build();

        let renderpass_create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&renderpass_attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(&dependencies)
            .build();

        let renderpass = base.device.create_render_pass(&renderpass_create_info, None).unwrap();

        // ================================================================
        // FRAMEBUFFERS
        // ================================================================

        let framebuffers: Vec<vk::Framebuffer> = base
            .present_image_views
            .iter()
            .map(|&present_image_view| {
                let framebuffer_attachments = [present_image_view, base.depth_image_view];
                let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(renderpass)
                    .attachments(&framebuffer_attachments)
                    .width(base.surface_resolution.width)
                    .height(base.surface_resolution.height)
                    .layers(1)
                    .build();

                base.device.create_framebuffer(&framebuffer_create_info, None).unwrap()
            })
            .collect();

        // ================================================================
        // MODELS
        // ================================================================

        let mesh_model = Model::from_file("./assets/room/viking_room.obj");

        // ================================================================
        // INDEX BUFFER
        // ================================================================

        let index_buffer = Buffer::new(
            &base,
            (std::mem::size_of::<u32>() * mesh_model.indices.len()) as u64,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            true
        );

        let mut slice = Align::new(index_buffer.ptr.unwrap(), align_of::<i32>() as u64, index_buffer.size);
        slice.copy_from_slice(&mesh_model.indices);

        index_buffer.unmap_memory(&base);

        // ================================================================
        // VERTEX BUFFER
        // ================================================================

        let vertex_buffer = Buffer::new(
            &base, 
            (std::mem::size_of::<Vertex>() * mesh_model.vertices.len()) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            true
        );

        let mut slice = Align::new(vertex_buffer.ptr.unwrap(), align_of::<Vertex>() as u64, vertex_buffer.size);
        slice.copy_from_slice(&mesh_model.vertices);

        vertex_buffer.unmap_memory(&base);

        // ================================================================
        // UNIFORM BUFFER
        // ================================================================

        let model = glm::rotate(&glm::identity(), glm::radians(&glm::vec1(45.0))[0], &glm::vec3(0.0, 0.0, 1.0));

        let view = glm::look_at(
            &glm::vec3(2.0, 2.0, 2.0),
            &glm::vec3(0.0, 0.0, 0.0),
            &glm::vec3(0.0, 0.0, 1.0)
        );

        let mut projection = glm::perspective_rh_zo(
            (base.surface_resolution.width as f32) / (base.surface_resolution.height as f32),
            glm::radians(&glm::vec1(45.0))[0],
            0.1,
            10.0
        );

        // glm was designed for OpenGL, so the Y axis has to be flipped
        projection[(1, 1)] *= -1.0;

        let uniform_color_buffer_data = UniformBufferObject {
            model, view, projection
        };

        let uniform_buffer = Buffer::new(
            &base,
            std::mem::size_of_val(&uniform_color_buffer_data) as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            true
        );

        let mut uniform_aligned_slice = Align::new(
            uniform_buffer.ptr.unwrap(),
            align_of::<glm::Vec4>() as u64,
            uniform_buffer.size
        );
        uniform_aligned_slice.copy_from_slice(&[uniform_color_buffer_data]);

        uniform_buffer.unmap_memory(&base);

        // ================================================================
        // TEXTURES
        // ================================================================

        let texture = Texture2D::load_from_file(&base, "./assets/room/viking_room.png").unwrap();

        // ================================================================
        // DESCRIPTORS
        // ================================================================

        let descriptor_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1
            },
        ];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&descriptor_sizes)
            .max_sets(1)
            .build();
        let descriptor_pool = base.device.create_descriptor_pool(&descriptor_pool_info, None).unwrap();

        // HINT: here's where you can change the binding of the buffers to the stages
        // for example: you can specify samplers working only on fragment, or both fragment and vertex
        // same thing with the UBO
        let desc_layout_bindings = [
            vk::DescriptorSetLayoutBinding {
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },

            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            }
        ];
        let descriptor_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&desc_layout_bindings)
            .build();

        let desc_set_layouts = [
            base.device.create_descriptor_set_layout(&descriptor_info, None).unwrap()
        ];

        let desc_alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&desc_set_layouts)
            .build();
        let descriptor_sets = base.device.allocate_descriptor_sets(&desc_alloc_info).unwrap();

        let uniform_color_buffer_descriptor = vk::DescriptorBufferInfo {
            buffer: uniform_buffer.buffer,
            offset: 0,
            range: std::mem::size_of_val(&uniform_color_buffer_data) as u64
        };

        let tex_descriptor = vk::DescriptorImageInfo { 
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image_view: texture.data.image_view,
            sampler: texture.data.sampler.unwrap()
        };

        let write_desc_sets = [
            vk::WriteDescriptorSet {
                dst_set: descriptor_sets[0],
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                p_buffer_info: &uniform_color_buffer_descriptor,
                ..Default::default()
            },

            vk::WriteDescriptorSet {
                dst_set: descriptor_sets[0],
                dst_binding: 1,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                p_image_info: &tex_descriptor,
                ..Default::default()
            },
        ];
        base.device.update_descriptor_sets(&write_desc_sets, &[]);

        // ================================================================
        // SHADERS
        // ================================================================

        let mut vertex_spv_file = Cursor::new(&include_bytes!("../shaders-cache/vert.spv")[..]);
        let mut frag_spv_file = Cursor::new(&include_bytes!("../shaders-cache/frag.spv")[..]);

        let vertex_code = read_spv(&mut vertex_spv_file).expect("Failed to read vertex shader");
        let vertex_shader_info = vk::ShaderModuleCreateInfo::builder().code(&vertex_code).build();

        let frag_code = read_spv(&mut frag_spv_file).expect("Failed to read fragment shader");
        let frag_shader_info = vk::ShaderModuleCreateInfo::builder().code(&frag_code).build();

        let vertex_shader_module = base.device.create_shader_module(&vertex_shader_info, None).expect("Vertex shader module error");
        let frag_shader_module = base.device.create_shader_module(&frag_shader_info, None).expect("Fragment shader module error");

        let layout_create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&desc_set_layouts).build();
        let pipeline_layout = base.device.create_pipeline_layout(&layout_create_info, None).unwrap();

        let shader_entry_name = CStr::from_bytes_with_nul_unchecked(b"main\0");
        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                module: vertex_shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                module: frag_shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];

        // ================================================================
        // FIXED FUNCTIONS
        // ================================================================

        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&Vertex::attribute_descriptions())
            .vertex_binding_descriptions(&[Vertex::binding_description()])
            .build();

        let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false) // TODO: what does it do?
            .build();

        // Viewport state

        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: base.surface_resolution.width as f32,
            height: base.surface_resolution.height as f32,
            min_depth: 0.0,
            max_depth: 1.0
        }];
        let scissors = [base.surface_resolution.into()];
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
            .scissors(&scissors)
            .viewports(&viewports)
            .build();

        // Rasterization state

        let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            ..Default::default()
        };

        // Multisample state

        let multisample_state_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .build();

        // Stencil state

        let noop_stencil_state = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            ..Default::default()
        };

        // Depth stencil state
        
        let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: 1,
            depth_write_enable: 1,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            front: noop_stencil_state,
            back: noop_stencil_state,
            max_depth_bounds: 1.0,
            ..Default::default()
        };

        // Color blend state

        let color_blend_attachment_states = [
            vk::PipelineColorBlendAttachmentState {
                blend_enable: 0,
                src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ZERO,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::RGBA
            }
        ];

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states)
            .build();

        // ================================================================
        // PIPELINE
        // ================================================================

        let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state).build();

        let graphics_pipeline_infos = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stage_create_infos)
            .vertex_input_state(&vertex_input_state_info)
            .input_assembly_state(&vertex_input_assembly_state_info)
            .viewport_state(&viewport_state_info)
            .rasterization_state(&rasterization_info)
            .multisample_state(&multisample_state_info)
            .depth_stencil_state(&depth_state_info)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_info)
            .layout(pipeline_layout)
            .render_pass(renderpass)
            .build();

        let graphics_pipelines = base
            .device
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[graphics_pipeline_infos],
                None
            )
            .unwrap();

        let graphic_pipeline = graphics_pipelines[0];

        // ================================================================
        // RENDER LOOP
        // ================================================================

        base.render_loop(|| {
            let (present_index, _) = base.swapchain_loader.acquire_next_image(
                base.swapchain,
                std::u64::MAX,
                base.present_complete_semaphore,
                vk::Fence::null()
            ).unwrap();

            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue { float32: [0.14, 0.15, 0.2, 0.0 ] }
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0
                    }
                }
            ];

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(renderpass)
                .framebuffer(framebuffers[present_index as usize])
                .render_area(base.surface_resolution.into())
                .clear_values(&clear_values)
                .build();

            record_submit_commandbuffer(
                &base.device,
                base.draw_command_buffer,
                base.draw_commands_reuse_fence,
                base.present_queue,
                &[vk::PipelineStageFlags::BOTTOM_OF_PIPE],
                &[base.present_complete_semaphore],
                &[base.rendering_complete_semaphore],
                |device, draw_command_buffer| {
                    device.cmd_begin_render_pass(draw_command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE);
                    device.cmd_bind_pipeline(draw_command_buffer, vk::PipelineBindPoint::GRAPHICS, graphic_pipeline);

                    device.cmd_set_viewport(draw_command_buffer, 0, &viewports);
                    device.cmd_set_scissor(draw_command_buffer, 0, &scissors);

                    device.cmd_bind_vertex_buffers(draw_command_buffer, 0, &[vertex_buffer.buffer], &[0]);
                    device.cmd_bind_index_buffer(draw_command_buffer, index_buffer.buffer, 0, vk::IndexType::UINT32);

                    device.cmd_bind_descriptor_sets(
                        draw_command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_layout,
                        0,
                        &descriptor_sets[..],
                        &[]
                    );

                    device.cmd_draw_indexed(
                        draw_command_buffer,
                        mesh_model.indices.len() as u32,
                        1, 0, 0, 0
                    );

                    device.cmd_end_render_pass(draw_command_buffer);
                }
            );

            let present_info = vk::PresentInfoKHR {
                wait_semaphore_count: 1,
                p_wait_semaphores: &base.rendering_complete_semaphore,
                swapchain_count: 1,
                p_swapchains: &base.swapchain,
                p_image_indices: &present_index,
                ..Default::default()
            };

            base.swapchain_loader.queue_present(base.present_queue, &present_info).unwrap();
        });

        // ================================================================
        // CLEANUP
        // ================================================================

        base.device.device_wait_idle().unwrap();

        for pipeline in graphics_pipelines {
            base.device.destroy_pipeline(pipeline, None);
        }

        base.device.destroy_pipeline_layout(pipeline_layout, None);
        base.device.destroy_shader_module(vertex_shader_module, None);
        base.device.destroy_shader_module(frag_shader_module, None);

        texture.data.destroy(&base);
        index_buffer.destroy(&base);
        uniform_buffer.destroy(&base);
        vertex_buffer.destroy(&base);

        for &descriptor_set_layout in desc_set_layouts.iter() {
            base.device.destroy_descriptor_set_layout(descriptor_set_layout, None);
        }

        base.device.destroy_descriptor_pool(descriptor_pool, None);

        for framebuffer in framebuffers {
            base.device.destroy_framebuffer(framebuffer, None);
        }

        base.device.destroy_render_pass(renderpass, None);
    }
}
