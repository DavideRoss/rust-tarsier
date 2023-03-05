use ash::extensions::{
    ext::DebugUtils,
    khr::{Surface, Swapchain}
};

use ash::{vk, Entry};
pub use ash::{Device, Instance};

use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;

pub struct Base {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,

    pub surface_loader: Surface,
    pub swapchain_loader: Swapchain,
    pub debug_utils_loader: DebugUtils,

    pub window: winit::window::Window,
    pub event_loop: RefCell<EventLoop<()>>,
    pub debug_callback: vk::DebugUtilsMessengerEXT,

    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,

    pub draw_commands_reuse_fence: vk::Fence,
    pub setup_commands_reuse_fence: vk::Fence
}

impl Base {
    pub fn new(window_width: u32, window_height: u32) -> Self {
        unsafe {
            let event_loop = EventLoop::new();
            let window = WindowBuilder::new()
                .with_title("Tarsier")
                .with_inner_size(winit::dpi::LogicalSize::new(
                    f64::from(window_width),
                    f64::from(window_height)
                ))
                .build(&event_loop)
                .unwrap();
            
            let entry = Entry::linked();
            let app_name = CStr::from_bytes_with_nul_unchecked(b"Tarsier\0");

            let layers_name = [
                CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0")
            ];
            let layers_names_raw: Vec<*const c_char> = layers_name.iter().map(|r| r.as_ptr()).collect();

            let mut extension_names = ash_window::enumerate_required_extensions(window.raw_display_handle())
                .unwrap()
                .to_vec();
            extension_names.push(DebugUtils::name().as_ptr());

            let app_info = vk::ApplicationInfo::builder()
                .application_name(app_name)
                .application_version(0)
                .engine_name(app_name)
                .engine_version(0)
                .api_version(vk::make_api_version(0, 1, 2, 0))
                .build();
            
            let create_flags = vk::InstanceCreateFlags::default();
            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names)
                .flags(create_flags)
                .build();

            let instance = entry.create_instance(&create_info, None).expect("Instance creation error");

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR |
                    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING |
                    vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL |
                    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION |
                    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                )
                .pfn_user_callback(Some(vulkan_debug_callback))
                .build();

            let debug_utils_loader = DebugUtils::new(&entry, &instance);
            let debug_callback = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap();

            let surface = ash_window::create_surface(
                &entry, 
                &instance, 
                window.raw_display_handle(), 
                window.raw_window_handle(), 
                None
            ).unwrap();

            let physical_devices = instance.enumerate_physical_devices().expect("Physical device error");
            let surface_loader = Surface::new(&entry, &instance);
            let (physical_device, queue_family_index) = physical_devices
                .iter()
                .find_map(|device| {
                    instance.get_physical_device_queue_family_properties(*device)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphics = info.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                            let supports_surface = surface_loader.get_physical_device_surface_support(*device, index as u32, surface).unwrap();

                            if supports_graphics && supports_surface {
                                Some((*device, index))
                            } else {
                                None
                            }
                        })
                }).expect("Could not find suitable physical device");

            let queue_family_index = queue_family_index as u32;

            let device_extension_names_raw = [
                Swapchain::name().as_ptr()
            ];

            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };

            let priorities = [1.0];

            let queue_info = vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities)
                .build();

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            let device = instance.create_device(physical_device, &device_create_info, None).unwrap();

            let present_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format = surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .unwrap()[0];

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .unwrap();

            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0 && desired_image_count > surface_capabilities.max_image_count {
                desired_image_count = surface_capabilities.max_image_count;
            }

            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window_width,
                    height: window_height
                },
                _ => surface_capabilities.current_extent
            };

            let pre_transform = if surface_capabilities.supported_transforms.contains(vk::SurfaceTransformFlagsKHR::IDENTITY) {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };

            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .unwrap();

            let present_mode = present_modes
                .iter().cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);

            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1)
                .build();

            let swapchain = swapchain_loader.create_swapchain(
                &swapchain_create_info,
                None
            ).unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index)
                .build();

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(2)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();

            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();

            let setup_command_buffer = command_buffers[0];
            let draw_command_buffer = command_buffers[1];

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1
                        })
                        .image(image)
                        .build();

                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();
            
            let device_memory_properties = instance.get_physical_device_memory_properties(physical_device);

            // ================================================================
            // DEPTH IMAGE
            // ================================================================

            let depth_image_create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D16_UNORM)
                .extent(surface_resolution.into())
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let depth_image = device.create_image(&depth_image_create_info, None).unwrap();
            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let depth_image_memory_index = find_memory_type_index(
                &depth_image_memory_req, 
                &device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL
            ).expect("Could not find suitable memory index for depth image");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index);

            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("Could not bind depth image memory");

            let fence_create_info = vk::FenceCreateInfo::builder()
                .flags(vk::FenceCreateFlags::SIGNALED)
                .build();

            let draw_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Could note create fence");

            let setup_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Could not create fence");

            record_submit_commandbuffer(
                &device, 
                setup_command_buffer, 
                setup_commands_reuse_fence, 
                present_queue, 
                &[], &[], &[], 
            |device, setup_command_buffer| {
                let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                    .image(depth_image)
                    .dst_access_mask(
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                    )
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .layer_count(1)
                            .level_count(1)
                            .build()
                    )
                    .build();

                device.cmd_pipeline_barrier(
                    setup_command_buffer, 
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE, 
                    vk::PipelineStageFlags::LATE_FRAGMENT_TESTS, 
                    vk::DependencyFlags::empty(), 
                    &[], &[], 
                    &[layout_transition_barriers]
                );
            });

            let depth_image_view_info = vk::ImageViewCreateInfo::builder()
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build()
                )
                .image(depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D)
                .build();

            let depth_image_view = device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            Base {
                event_loop: RefCell::new(event_loop),

                entry, instance, device,
                queue_family_index,
                physical_device,
                device_memory_properties,
                
                window,
                surface_loader,
                surface_format,
                present_queue,
                surface_resolution,
                swapchain_loader,
                swapchain,
                present_images,
                present_image_views,
                pool,
                
                draw_command_buffer,
                setup_command_buffer,

                depth_image,
                depth_image_memory,
                depth_image_view,

                present_complete_semaphore,
                rendering_complete_semaphore,

                draw_commands_reuse_fence,
                setup_commands_reuse_fence,

                surface,

                debug_callback,
                debug_utils_loader,
            }
        }
    }

    pub fn render_loop<F: Fn()>(&self, f: F) {
        // let present_info = vk::PresentInfoKHR::builder()
        //     .wait_semaphores(&[self.rendering_complete_semaphore])
        //     .swapchains(&[self.swapchain])
        //     .image_indices(&[0]) // TODO: this value should be the swapimage index
        //     .build();

        // let result = unsafe {
        //     self.swapchain_loader.queue_present(self.present_queue, &present_info).unwrap();
        // }
        // println!("queue present result: {}", result);

        // TODO: handle minimize to icon case too
        self.event_loop
            .borrow_mut()
            .run_return(|event, _, control_flow| {
                *control_flow = ControlFlow::Poll;
                match event {
                    // On esc -> close the window
                    Event::WindowEvent {
                        event:
                            WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                                input:
                                    KeyboardInput {
                                        state: ElementState::Pressed,
                                        virtual_keycode: Some(VirtualKeyCode::Escape),
                                        ..
                                    },
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,

                    // On resize -> rebuild swapchain
                    Event::WindowEvent {
                        event:
                            WindowEvent::Resized(_),
                        ..
                    } => println!("Window resized!"),

                    // On clear -> call render loop
                    Event::MainEventsCleared => f(),

                    _ => (),
                }
            });
    }

    pub unsafe fn recreate_swapchain(&mut self) {
        self.device.device_wait_idle().unwrap();

        // Cleanup
        self.device.free_memory(self.depth_image_memory, None);
        self.device.destroy_image_view(self.depth_image_view, None);
        self.device.destroy_image(self.depth_image, None);
        // TODO: not complete! Will trigger errors here

        let dimensions = [
            self.window.inner_size().width,
            self.window.inner_size().height
        ];
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_semaphore(self.present_complete_semaphore, None);
            self.device.destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device.destroy_fence(self.draw_commands_reuse_fence, None);
            self.device.destroy_fence(self.setup_commands_reuse_fence, None);

            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);

            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }

            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_utils_loader.destroy_debug_utils_messenger(self.debug_callback, None);
            self.instance.destroy_instance(None);
        }
    }
}

pub fn find_memory_type_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _)| index as _)
}

#[allow(clippy::too_many_arguments)]
pub fn record_submit_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    command_buffer_reuse_fence: vk::Fence,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F
) {
    unsafe {
        device
            .wait_for_fences(&[command_buffer_reuse_fence], true, std::u64::MAX)
            .expect("Wait for fence failed");

        device
            .reset_fences(&[command_buffer_reuse_fence])
            .expect("Reset fences failed");

        device
            .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES)
            .expect("Reset command buffer failed");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Unable to begin command buffer");

        f(device, command_buffer);

        device.end_command_buffer(command_buffer).expect("Unable to end command buffer");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores)
            .build();

        device
            .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
            .expect("Unable to submit queue");
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callkback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void
) -> vk::Bool32 {
    let callback_data = *p_callkback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{message_severity:?}: {message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = std::mem::zeroed();
            std::ptr::addr_of!(b.$field) as isize - std::ptr::addr_of!(b) as isize
        }
    }};
}