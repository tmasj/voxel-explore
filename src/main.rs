use ash::khr::swapchain;
use ash::vk::{Extent2D, Handle, ImageViewCreateInfo, Pipeline};
use ash::{vk, Entry};
use glfw::{Action, Context, Glfw, Key, PWindow, WindowEvent, WindowHint, GlfwReceiver};
use glfw::fail_on_errors;
use std::ffi::{CStr, c_char};
use std::ptr::null;
use std::{fs, io};
use std::fs::File;

type Events = GlfwReceiver<(f64, WindowEvent)>;

struct VulkanContext {
    _entry: Entry,
    instance: ash::Instance,
    surface: vk::SurfaceKHR,
    surface_loader: ash::khr::surface::Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
    swapchain: Swapchain,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline
}

struct SwapchainImage {
    image: vk::Image,
    view: vk::ImageView,
    command_buffer: vk::CommandBuffer
}

struct Swapchain {
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<SwapchainImage>,
    swapchain_extent: vk::Extent2D,
    swapchain_format: vk::Format,
    command_resources: vk::CommandPool
}


struct SyncPrimitives {
    image_available: Vec<vk::Semaphore>,
    render_finished: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
}

fn create_window(glfw_handle: &mut Glfw) -> (PWindow, Events) {
    dbg!(glfw_handle.vulkan_supported());
    if !glfw_handle.vulkan_supported() {
        panic!("vulkan not supported!!");
    }

    glfw_handle.window_hint(WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
    glfw_handle.window_hint(WindowHint::Decorated(true));
    glfw_handle.window_hint(WindowHint::Resizable(true));
    
    let (mut window, events) = glfw_handle
        .create_window(800, 600, "VOXELEXPLOR", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");
    
    window.set_key_polling(true);
    window.set_size_polling(true);
    window.set_framebuffer_size_polling(true);
    (window, events)
}

fn create_vulkan_instance(glfw_handle: &Glfw, entry: &Entry) -> ash::Instance {
    let app_info = vk::ApplicationInfo::default()
        .application_name(CStr::from_bytes_with_nul(b"Voxel Explore\0").unwrap())
        .application_version(vk::make_api_version(0, 1, 0, 0))
        .engine_name(CStr::from_bytes_with_nul(b"No Engine\0").unwrap())
        .engine_version(vk::make_api_version(0, 1, 0, 0))
        .api_version(vk::API_VERSION_1_0);

    // Owns the values pointed to by extension_names_prt
    let mut extension_names = glfw_handle
        .get_required_instance_extensions()
        .expect("Failed to get required extensions")
        .iter()
        .map(|s| std::ffi::CString::new(s.as_str()).unwrap() )
        .collect::<Vec<_>>();
    let extension_names_ptr: Vec<*const c_char> = extension_names
        .iter()
        .map(|s| s.as_ptr() )
        .collect();

    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&extension_names_ptr);

    let available = unsafe { entry.enumerate_instance_extension_properties(None).unwrap() };
    println!("Available extensions:");
    for ext in &available {
        println!("  {}", unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) }.to_str().unwrap());
    }
    println!("\nRequested extensions:");
    for &ext in &extension_names_ptr {
        println!("  {}", unsafe { CStr::from_ptr(ext) }.to_str().unwrap());
    }

    unsafe { entry.create_instance(&create_info, None).unwrap() }
}

fn create_surface(
    window: &mut PWindow,
    entry: &Entry,
    instance: &ash::Instance,
) -> (vk::SurfaceKHR, ash::khr::surface::Instance) {
    let mut surface_handle: glfw::ffi::VkSurfaceKHR = std::ptr::null_mut();
    let result: glfw::ffi::VkResult;
    unsafe {
        result = window.create_window_surface(
            instance.handle().as_raw() as *mut glfw::ffi::VkInstance_T,
            std::ptr::null(),
            &mut surface_handle,
        );
    }
    assert_eq!(result, vk::Result::SUCCESS.as_raw());
    
    let surface = vk::SurfaceKHR::from_raw(surface_handle as u64);
    let surface_loader = ash::khr::surface::Instance::new(entry, instance);
    
    (surface, surface_loader)
}

fn pick_physical_device(
    instance: &ash::Instance,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> (vk::PhysicalDevice, u32) {
    let physical_devices = unsafe { instance.enumerate_physical_devices().unwrap() };
    let physical_device = physical_devices[0];

    let queue_families = unsafe {
        instance.get_physical_device_queue_family_properties(physical_device)
    };
    
    let queue_family_index = queue_families
        .iter()
        .enumerate()
        .find(|(i, qf)| {
            qf.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && unsafe {
                    surface_loader
                        .get_physical_device_surface_support(physical_device, *i as u32, surface)
                        .unwrap()
                }
        })
        .map(|(i, _)| i as u32)
        .expect("No suitable queue family");

    (physical_device, queue_family_index)
}

fn create_logical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
) -> (ash::Device, vk::Queue) {
    let queue_priorities = [1.0f32];
    let queue_create_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities);

    let device_extension_names = [ash::khr::swapchain::NAME.as_ptr()];
    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_create_info))
        .enabled_extension_names(&device_extension_names);

    let device = unsafe {
        instance
            .create_device(physical_device, &device_create_info, None)
            .unwrap()
    };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    (device, queue)
}

fn create_swapchain(
    instance: &ash::Instance,
    device: &ash::Device,
    physical_device: vk::PhysicalDevice,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    old_swapchain: Option<vk::SwapchainKHR>
) -> Swapchain {
    let surface_caps = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(physical_device, surface)
            .unwrap()
    };
    let swapchain_extent = surface_caps.current_extent; // Possibly an unhandled special value https://docs.vulkan.org/refpages/latest/refpages/source/VkSurfaceCapabilitiesKHR.html see also https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain
    let surface_format = unsafe {
        surface_loader
            .get_physical_device_surface_formats(physical_device, surface)
            .unwrap()[0]
    };
    let swapchain_format: vk::Format = surface_format.format;

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(surface_caps.min_image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(surface_caps.current_extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE) // See also https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain
        .pre_transform(surface_caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(vk::PresentModeKHR::FIFO) // Ought to query for supported present modes first
        .clipped(true)
        .old_swapchain(old_swapchain.unwrap_or(vk::SwapchainKHR::null())); //Dealloc helper https://vulkan-tutorial.com/Drawing_a_triangle/Swap_chain_recreation

    let swapchain_loader = ash::khr::swapchain::Device::new(instance,device);
    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&swapchain_create_info, None)
            .unwrap()
    };

    // The vulkan context holds a present queue. That queue likely the same as this one but not necessarily
    let queue_family_index = unsafe {
        instance
            .get_physical_device_queue_family_properties(physical_device)
            .iter()
            .position(|props| props.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .unwrap() as u32
    };

    let pool_create_info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family_index);
    let command_resources = unsafe { device.create_command_pool(&pool_create_info, None).unwrap() };

    
    let images_from_loader: Vec<vk::Image>;
    let swapchain_images: Vec<SwapchainImage>;
    unsafe {
        images_from_loader = swapchain_loader.get_swapchain_images(swapchain).unwrap();
        let buffer_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_resources)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(images_from_loader.len() as u32);
        let command_buffers = unsafe { device.allocate_command_buffers(&buffer_alloc_info).unwrap() };

        swapchain_images = images_from_loader
            .iter()
            .zip(command_buffers.iter())
            .map(|(&image, &buffer)| {
                let image_create_info: vk::ImageViewCreateInfo = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(swapchain_format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1) 
                );
                SwapchainImage{
                    image: image,
                    view: device.create_image_view(
                        &image_create_info,
                        None
                    ).unwrap(),
                    command_buffer: buffer
                }
            }).collect::<Vec<_>>();
        }
    

    Swapchain {
        swapchain_loader,
        swapchain,
        swapchain_images,
        swapchain_extent,
        swapchain_format,
        command_resources
    }
}

fn init_vulkan(glfw_handle: &Glfw, window: &mut PWindow) -> VulkanContext {
    let entry = unsafe { Entry::load().unwrap() };
    let instance = create_vulkan_instance(glfw_handle, &entry);
    let (surface, surface_loader) = create_surface(window, &entry, &instance);
    let (physical_device, queue_family_index) =
        pick_physical_device(&instance, &surface_loader, surface);
    let (device, queue) = create_logical_device(&instance, physical_device, queue_family_index);
    let swapchain = create_swapchain(
        &instance,
        &device,
        physical_device,
        &surface_loader,
        surface,
        None
    );

    let render_pass: vk::RenderPass = create_render_pass(&device, &swapchain);
    let (graphics_pipeline, pipeline_layout) = create_graphics_pipeline(&device, &swapchain, render_pass);
    

    VulkanContext {
        _entry: entry,
        instance,
        surface,
        surface_loader,
        physical_device,
        device,
        queue,
        queue_family_index,
        swapchain,
        render_pass,
        pipeline_layout,
        graphics_pipeline
    }
}

fn event_loop(
    glfw_handle: &mut Glfw,
    window: &mut PWindow,
    events: Events,
    _vk_ctx: &mut VulkanContext,
) {
    let mut last_size = window.get_size();
    while !window.should_close() {
        glfw_handle.poll_events();
        let current_size = window.get_size();
        // if current_size != last_size {
        //     println!("Window resized: {:?}", current_size);
        //     // handle resize here
        //     last_size = current_size;
        // }
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Key(Key::W, _, Action::Press, _) => {
                    println!("W!");
                }
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true)
                }
                WindowEvent::Size(width, height) => {
                    
                }
                WindowEvent::FramebufferSize(width, height) => {
                    _vk_ctx.swapchain = create_swapchain(&_vk_ctx.instance, &_vk_ctx.device, _vk_ctx.physical_device, &_vk_ctx.surface_loader, _vk_ctx.surface, Some(_vk_ctx.swapchain.swapchain) );
                }
                _ => {
                }
            }
        }
    }
}

fn cleanup_vulkan(vk_ctx: VulkanContext) {
    unsafe {
        vk_ctx
            .swapchain
            .swapchain_loader
            .destroy_swapchain(vk_ctx.swapchain.swapchain, None);
        for image_rec in vk_ctx.swapchain.swapchain_images {
            
            vk_ctx.device.destroy_image_view(image_rec.view, None);
        }
        vk_ctx.device.destroy_device(None);
        vk_ctx.surface_loader.destroy_surface(vk_ctx.surface, None);
        vk_ctx.instance.destroy_instance(None);
        vk_ctx.device.destroy_pipeline_layout(vk_ctx.pipeline_layout, None);
        vk_ctx.device.destroy_render_pass(vk_ctx.render_pass, None);
        vk_ctx.device.destroy_pipeline(vk_ctx.graphics_pipeline, None);
    }
}

fn shader_mod_from_spv_path(device: &ash::Device, pathname: impl AsRef<std::path::Path>) -> vk::ShaderModule {
    let mut flhndl = File::open(pathname).unwrap();
    let shader_code = ash::util::read_spv(&mut flhndl).unwrap();
    let create_info = vk::ShaderModuleCreateInfo::default()
        .code(&shader_code);
    
    unsafe {
        return device.create_shader_module(&create_info, None).unwrap();
    }
}

fn create_render_pass(device: &ash::Device, swapchain: &Swapchain) -> vk::RenderPass {
    let color_attachment = vk::AttachmentDescription::default()
        .format(swapchain.swapchain_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
    // The Vulkan domain jargon is literally 'attachments array'
    let attachments_array = [color_attachment]; 
    let color_attachment_ref = vk::AttachmentReference::default()
        .attachment(0) // the index of 'color_attachment', our one description
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let attachments_references = [color_attachment_ref];

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&attachments_references);
    let subpasses = [subpass];

    let render_pass_create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments_array)
        .subpasses(&subpasses);

    let render_pass: vk::RenderPass;
    unsafe {
        render_pass = device.create_render_pass(&render_pass_create_info, None).unwrap();
    }

    render_pass

}

fn create_graphics_pipeline(device: &ash::Device, swapchain: &Swapchain, render_pass: vk::RenderPass) -> (vk::Pipeline, vk::PipelineLayout) {
    // no shader code constants yet
    let specialization_info = vk::SpecializationInfo::default();

    // Vertex Shader setup
    let vert_shader_mod = shader_mod_from_spv_path(&device, "vert.spv");
    let vert_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vert_shader_mod)
        .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
        .specialization_info(&specialization_info);

    // Frag Shader setup
    let frag_shader_mod = shader_mod_from_spv_path(&device, "frag.spv");
    let frag_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(frag_shader_mod)
        .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
        .specialization_info(&specialization_info);

    let shader_stages = [vert_create_info, frag_create_info];

    let dynamic_states: [vk::DynamicState; 2] = [
        vk::DynamicState::VIEWPORT,
        vk::DynamicState::SCISSOR
    ];
    let dynamic_state_create_info = vk::PipelineDynamicStateCreateInfo::default()
        .dynamic_states(&dynamic_states);

    let vertex_binding_descriptions = [];
    let vertex_attribute_descriptions = [];
    let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&vertex_binding_descriptions)
        .vertex_attribute_descriptions(&vertex_attribute_descriptions);

    let pipeline_input_create_info = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    let viewport = vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(swapchain.swapchain_extent.width as f32)
        .height(swapchain.swapchain_extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0);
    let scissor = vk::Rect2D::default()
        .offset(vk::Offset2D::default())
        .extent(swapchain.swapchain_extent);
    let viewports = [viewport];
    let scissors = [scissor];
    let viewport_create_info = vk::PipelineViewportStateCreateInfo::default()
        .viewports(&viewports)
        .scissors(&scissors);

    let rasterization_create_info = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false)
        .depth_bias_constant_factor(0.0)
        .depth_bias_clamp(0.0)
        .depth_bias_slope_factor(0.0);

    // For now multisampling is off, but this has to do with anti-aliasing
    let sample_masks = [];
    let multisample_create_info = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .min_sample_shading(1.0)
        .sample_mask(&sample_masks)
        .alpha_to_coverage_enable(false)
        .alpha_to_one_enable(false);

    let depthstencil_create_info = vk::PipelineDepthStencilStateCreateInfo::default();

    // Since we have just one framebuffer, we have just one ColorBlendAttachmentState
    let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD);
    let blend_attachments = [blend_attachment];
    let blend_create_info = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&blend_attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    let set_layouts = [];
    let push_constant_ranges = [];
    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&set_layouts)
        .push_constant_ranges(&push_constant_ranges);
    
    let pipeline_layout;
    unsafe {
        pipeline_layout = device.create_pipeline_layout(&pipeline_layout_create_info, None).unwrap();
    }

    let graphics_pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input_create_info)
        .viewport_state(&viewport_create_info)
        .input_assembly_state(&pipeline_input_create_info)
        .rasterization_state(&rasterization_create_info)
        .multisample_state(&multisample_create_info)
        .depth_stencil_state(&depthstencil_create_info)
        .color_blend_state(&blend_create_info)
        .dynamic_state(&dynamic_state_create_info)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0)
        .base_pipeline_handle(vk::Pipeline::null())
        .base_pipeline_index(-1);

    let pipeline_create_infos = [graphics_pipeline_create_info];
    let graphics_pipeline: Vec<vk::Pipeline>;
    unsafe {
        graphics_pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None).unwrap();
    }

    if !(graphics_pipeline.len() == 1) {
        panic!("I thought there would be exactly one graphics pipeline...");
    }

    return (graphics_pipeline[0], pipeline_layout);

}

fn main() {
    let platform = if cfg!(target_os = "linux") {
        // Could add env checks here for WSL detection
        glfw::Platform::X11
    } else if cfg!(target_os = "macos") {
        glfw::Platform::MacOS
    } else if cfg!(target_os = "windows") {
        glfw::Platform::Win32
    } else {
        glfw::Platform::Any // fallback
    };

    glfw::init_hint(
        glfw::InitHint::Platform(platform)
    );
    let mut glfwh = glfw::init(fail_on_errors!()).unwrap();
    let (mut window, events) = create_window(&mut glfwh);
    let mut vk_ctx = init_vulkan(&glfwh, &mut window);

    event_loop(&mut glfwh, &mut window, events, &mut vk_ctx);

    cleanup_vulkan(vk_ctx);
}