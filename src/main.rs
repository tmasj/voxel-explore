#![allow(unused_imports)]
#![allow(dead_code)]
use ash::vk::{
    AttachmentDescription, DebugUtilsMessengerCreateInfoEXT, DescriptorSetLayout, DeviceMemory,
    Handle, MemoryAllocateInfo,
};
use ash::{Entry, vk};
use glam;
use glam::{Mat4, Vec3};
use glfw::fail_on_errors;
use glfw::{Action, Glfw, GlfwReceiver, Key, PWindow, WindowEvent, WindowHint};
use std::char::MAX;
use std::ffi::{CStr, c_char};
use std::fs::File;
use std::time;
mod shader;

type Events = GlfwReceiver<(f64, WindowEvent)>;

const MAX_FRAMES_IN_FLIGHT: usize = 5;

struct VulkanContext {
    _entry: Entry,
    program_start: time::Instant,
    last_frame_instant: time::Instant,
    instance: ash::Instance,
    debug_msg_handler: vk::DebugUtilsMessengerEXT,
    debug_loader: ash::ext::debug_utils::Instance,
    surface: vk::SurfaceKHR,
    surface_loader: ash::khr::surface::Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
    swapchain: Swapchain,
    // The Vulkan tutorial (rust version, https://kylemayes.github.io/vulkanalia/model/depth_buffering.html) says:
    // We only need a single depth image, because only one draw operation is running at once.
    // However I think that's not true, at least in my case, so I will err on the side of using 1 DepthBufferSystem per framebuffer/swapchain image
    depth_buffers: Vec<DepthBufferSystem>,
    render_pass: vk::RenderPass,
    pipeline_system: GraphicsPipeline,
    // TODO one framebuffer per swapchain image. But it requires both a render pass and a swapchain. Currently render pass requires swapchain, and Swapchain needs to make a SwapchainImage
    // Refactor so the render pass is created in the swapchain to avoid circular dependency
    framebuffers: Vec<vk::Framebuffer>,
    sync_primitives: [SyncPrimitives; MAX_FRAMES_IN_FLIGHT],
    bufs: BufferSystemIndexed,
}

struct GraphicsPipeline {
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    shader_mod: Vec<vk::ShaderModule>,
}

struct SwapchainImage {
    image: vk::Image,
    image_index: u32,
    view: vk::ImageView,
    render_finished: vk::Semaphore,
}

struct Swapchain {
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<SwapchainImage>,
    swapchain_extent: vk::Extent2D,
    swapchain_format: vk::Format,
    command_resources: vk::CommandPool,
    command_buffers: [vk::CommandBuffer; MAX_FRAMES_IN_FLIGHT],
}

struct SyncPrimitives {
    image_available: vk::Semaphore,
    frame_in_flight: vk::Fence,
}

struct BufferSystemIndexed {
    devloc_vertex: vk::Buffer,
    vertex_mem: vk::DeviceMemory,
    devloc_index: vk::Buffer,
    index_mem: vk::DeviceMemory,
    unibufs: Vec<UniformBufSubsys>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
}

struct UniformBufSubsys {
    uniform_buffer: vk::Buffer,
    unif_mem: vk::DeviceMemory,
    mapped: *mut UniformBufferObject,
    desc_set: vk::DescriptorSet,
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
    window.set_close_polling(true);
    (window, events)
}

fn create_vulkan_instance(glfw_handle: &Glfw, entry: &Entry) -> ash::Instance {
    let app_info = vk::ApplicationInfo::default()
        .application_name(CStr::from_bytes_with_nul(b"Voxel Explore\0").unwrap())
        .application_version(vk::make_api_version(0, 1, 0, 0))
        .engine_name(CStr::from_bytes_with_nul(b"No Engine\0").unwrap())
        .engine_version(vk::make_api_version(0, 1, 0, 0))
        .api_version(vk::API_VERSION_1_3);

    // Owns the values pointed to by extension_names_prt
    let mut extension_names = glfw_handle
        .get_required_instance_extensions()
        .expect("Failed to get required extensions")
        .iter()
        .map(|s| std::ffi::CString::new(s.as_str()).unwrap())
        .collect::<Vec<_>>();

    // With validation enabled, sets up debug utils
    extension_names.push(std::ffi::CString::from(ash::ext::debug_utils::NAME));

    let extension_names_ptr: Vec<*const c_char> =
        extension_names.iter().map(|s| s.as_ptr()).collect();

    let layer_names: [&CStr; 1];
    unsafe {
        layer_names = [CStr::from_bytes_with_nul_unchecked(
            b"VK_LAYER_KHRONOS_validation\0",
        )];
    }
    let layer_names_raw: Vec<*const i8> = layer_names.iter().map(|name| name.as_ptr()).collect();

    // Enable printf debugging
    // https://github.com/KhronosGroup/Vulkan-Samples/blob/e6ada08f110de050636617a08821368efa7cd23b/samples/extensions/shader_debugprintf/README.adoc#L45
    let enabled_validation_features = [vk::ValidationFeatureEnableEXT::DEBUG_PRINTF];
    let mut next_validation_features = vk::ValidationFeaturesEXT::default()
        .enabled_validation_features(&enabled_validation_features);
    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&extension_names_ptr)
        .enabled_layer_names(&layer_names_raw)
        .push_next(&mut next_validation_features);

    let available = unsafe { entry.enumerate_instance_extension_properties(None).unwrap() };
    println!("Available extensions:");
    for ext in &available {
        println!(
            "  {}",
            unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) }
                .to_str()
                .unwrap()
        );
    }
    println!("\nRequested extensions:");
    for &ext in &extension_names_ptr {
        println!("  {}", unsafe { CStr::from_ptr(ext) }.to_str().unwrap());
    }

    let instance: ash::Instance;
    unsafe { instance = entry.create_instance(&create_info, None).unwrap() }

    return instance;
}

fn create_debug_instance(
    entry: &Entry,
    instance: &ash::Instance,
) -> (ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT) {
    // Set up debugging. There's a separate debug callback mechanisms for setting up the instance. This general purpose debug messenger registers a callback for everything else
    unsafe extern "system" fn debug_callback(
        flags: vk::DebugUtilsMessageSeverityFlagsEXT,
        _type: vk::DebugUtilsMessageTypeFlagsEXT,
        data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _user_data: *mut std::ffi::c_void,
    ) -> vk::Bool32 {
        let data = unsafe { *data };
        let message: &CStr = unsafe { data.message_as_c_str() }.unwrap_or_default();
        type F = vk::DebugUtilsMessageSeverityFlagsEXT;
        if flags >= F::ERROR {
            eprintln!("[{:?}] {:?}", flags, message);
        } else if flags >= F::WARNING {
            eprintln!("[{:?}] {:?}", flags, message);
        } else if flags >= F::INFO {
            eprintln!("[{:?}] {:?}", flags, message);
        } else {
            eprintln!("[{:?}] {:?}", flags, message);
        }
        return 0;
    }
    let callback: vk::PFN_vkDebugUtilsMessengerCallbackEXT = Some(debug_callback);
    let severity_flags = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
    let message_types = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE;
    let debug_messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(severity_flags)
        .flags(vk::DebugUtilsMessengerCreateFlagsEXT::empty())
        .message_type(message_types)
        .user_data(std::ptr::null_mut())
        .pfn_user_callback(callback);

    let debug_loader: ash::ext::debug_utils::Instance;
    let messenger: vk::DebugUtilsMessengerEXT;
    unsafe {
        debug_loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
        messenger = debug_loader
            .create_debug_utils_messenger(&debug_messenger_info, None)
            .unwrap();
    }

    return (debug_loader, messenger);
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

    let queue_families =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

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
    old_swapchain: Option<vk::SwapchainKHR>,
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

    let mic = Ord::min(
        surface_caps.min_image_count + 1,
        if surface_caps.max_image_count == 0 {
            u32::MAX
        } else {
            surface_caps.max_image_count
        },
    );

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(mic)
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

    let swapchain_loader = ash::khr::swapchain::Device::new(instance, device);
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

    let pool_create_info: vk::CommandPoolCreateInfo<'_> = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(queue_family_index);
    let command_resources = unsafe { device.create_command_pool(&pool_create_info, None).unwrap() };

    let images_from_loader: Vec<vk::Image>;
    let swapchain_images: Vec<SwapchainImage>;
    let command_buffers: [vk::CommandBuffer; MAX_FRAMES_IN_FLIGHT];
    unsafe {
        images_from_loader = swapchain_loader.get_swapchain_images(swapchain).unwrap();
        let buffer_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_resources)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);
        let command_buffers_vec = device.allocate_command_buffers(&buffer_alloc_info).unwrap();
        command_buffers = std::array::from_fn(|i| command_buffers_vec[i]);
        let sem_create_info = vk::SemaphoreCreateInfo::default();

        swapchain_images = images_from_loader
            .iter()
            .enumerate()
            .map(|(idx, &image)| {
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
                            .layer_count(1),
                    );

                SwapchainImage {
                    image: image,
                    image_index: idx as u32,
                    view: device.create_image_view(&image_create_info, None).unwrap(),
                    render_finished: device.create_semaphore(&sem_create_info, None).unwrap(),
                }
            })
            .collect::<Vec<_>>();
    }

    Swapchain {
        swapchain_loader,
        swapchain,
        swapchain_images,
        swapchain_extent,
        swapchain_format,
        command_resources,
        command_buffers,
    }
}

fn destroy_swapchain_system(vk_ctx: &VulkanContext) {
    let fences: [vk::Fence; MAX_FRAMES_IN_FLIGHT] = vk_ctx
        .sync_primitives
        .each_ref()
        .map(|sp| sp.frame_in_flight);
    unsafe {
        // clear the graphics pipeline
        // Technically, a wait_for_fences could deadlock here if a submit failed.
        // The Vulkan spec demands that a failing queue_submit() cannot alter resource states.
        // device_wait_idle here would simply stall work until gpu compute finishes, which is unbounded
        // Not doing anything with the fences risks waiting on them unsignalled.
        // So since I can't just signal them from host side TODO I need to recreate the fences (safe after the queue completed whether or not signalled).

        vk_ctx.device.queue_wait_idle(vk_ctx.queue).unwrap();
    }
    unsafe {
        for dbsref in &vk_ctx.depth_buffers {
            DepthBufferSystem::destroy(dbsref, &vk_ctx.device);
        }
        for framebuffer in &vk_ctx.framebuffers {
            vk_ctx.device.destroy_framebuffer(*framebuffer, None);
        }
        for image_rec in &vk_ctx.swapchain.swapchain_images {
            vk_ctx.device.destroy_image_view(image_rec.view, None);
            vk_ctx
                .device
                .destroy_semaphore(image_rec.render_finished, None);
        }
        vk_ctx
            .device
            .destroy_command_pool(vk_ctx.swapchain.command_resources, None);

        vk_ctx
            .swapchain
            .swapchain_loader
            .destroy_swapchain(vk_ctx.swapchain.swapchain, None);
    }
}

fn init_vulkan(glfw_handle: &Glfw, window: &mut PWindow) -> VulkanContext {
    let entry = unsafe { Entry::load().unwrap() };
    let program_start = time::Instant::now();
    let last_frame_instant = time::Instant::now();
    let instance = create_vulkan_instance(glfw_handle, &entry);
    let (debug_loader, debug_msg_handler) = create_debug_instance(&entry, &instance);
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
        None,
    );

    let descriptor_set_layout = UniformBufferObject::descriptor_set_layout(&device);
    let (descriptor_pool, descsets) =
        create_descriptor_sets_in_new_pool(&device, descriptor_set_layout);

    let render_pass: vk::RenderPass = create_render_pass(&device, &swapchain);
    let depth_buffers: Vec<DepthBufferSystem> = (0..swapchain.swapchain_images.len())
        .map(|_i| DepthBufferSystem::new(&device, &instance, &swapchain, physical_device))
        .collect();
    let pipeline_system: GraphicsPipeline =
        create_graphics_pipeline(&device, &swapchain, render_pass, descriptor_set_layout);
    let framebuffers: Vec<vk::Framebuffer> =
        create_framebuffers(&device, &swapchain, render_pass, &depth_buffers);
    let sync_primitives: [SyncPrimitives; MAX_FRAMES_IN_FLIGHT] =
        std::array::from_fn(|_i| create_sync_primitives(&device));

    let uniform_bufs: Vec<UniformBufSubsys> = (0..MAX_FRAMES_IN_FLIGHT)
        .map(|_i| {
            UniformBufferObject::uniform_buffer(&device, &instance, physical_device, descsets[_i])
        })
        .collect();

    let geom_vert = triangle_vertices_indexed();
    let geom_ind = triangle_geom_indices();
    let (vertex_buffer, devmem_vertex) =
        create_device_local_vertex_buffer(&device, &instance, physical_device, &geom_vert);
    let (index_buffer, devmem_index) =
        create_device_local_index_buffer(&device, &instance, physical_device, &geom_ind);
    let bufs = BufferSystemIndexed {
        devloc_vertex: vertex_buffer,
        vertex_mem: devmem_vertex,
        devloc_index: index_buffer,
        index_mem: devmem_index,
        unibufs: uniform_bufs,
        descriptor_pool: descriptor_pool,
        descriptor_set_layout: descriptor_set_layout,
    };

    VulkanContext {
        _entry: entry,
        program_start,
        last_frame_instant,
        instance,
        debug_msg_handler,
        debug_loader,
        surface,
        surface_loader,
        physical_device,
        device,
        queue,
        queue_family_index,
        swapchain,
        depth_buffers,
        render_pass,
        pipeline_system,
        framebuffers,
        sync_primitives,
        bufs,
    }
}

fn recreate_swapchain(vk_ctx: &mut VulkanContext) {
    dbg!("Recreating Swapchain");
    destroy_swapchain_system(vk_ctx);

    vk_ctx.swapchain = create_swapchain(
        &vk_ctx.instance,
        &vk_ctx.device,
        vk_ctx.physical_device,
        &vk_ctx.surface_loader,
        vk_ctx.surface,
        None,
    );
    vk_ctx.depth_buffers = (0..vk_ctx.swapchain.swapchain_images.len())
        .map(|_i| {
            DepthBufferSystem::new(
                &vk_ctx.device,
                &vk_ctx.instance,
                &vk_ctx.swapchain,
                vk_ctx.physical_device,
            )
        })
        .collect();
    vk_ctx.framebuffers = create_framebuffers(
        &vk_ctx.device,
        &vk_ctx.swapchain,
        vk_ctx.render_pass,
        &vk_ctx.depth_buffers,
    ); // Reusing this render pass as-is, though it doesn't handle all cases
}

fn event_loop(
    glfw_handle: &mut Glfw,
    window: &mut PWindow,
    events: Events,
    _vk_ctx: &mut VulkanContext,
) {
    let mut frameidx = 0;
    const FRAME_DRAW_RETRY_CAP: u8 = 100;
    let mut frame_draw_retries: [u8; MAX_FRAMES_IN_FLIGHT] = [0; MAX_FRAMES_IN_FLIGHT];

    load_vertex_data_via_staging_buffer(_vk_ctx, &triangle_vertices_indexed());
    load_index_data_via_staging_buffer(_vk_ctx, &triangle_geom_indices());

    while !window.should_close() {
        frame_draw_retries[frameidx] += 1;
        if frame_draw_retries[frameidx] > FRAME_DRAW_RETRY_CAP {
            panic!("The frame draw retry cap exceeded");
        }

        let mut drawrslt = draw_frame_by_index(_vk_ctx, frameidx);
        match drawrslt {
            Ok(_) => {
                frameidx = (frameidx + 1) % MAX_FRAMES_IN_FLIGHT;
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                recreate_swapchain(_vk_ctx);
                drawrslt = draw_frame_by_index(_vk_ctx, frameidx);
                continue;
            }
            othererr => panic!("Failed to draw frame: {:?}", othererr),
        };
        frame_draw_retries[frameidx] = 0;

        glfw_handle.poll_events();

        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Key(Key::W, _, Action::Press, _) => {
                    println!("W!");
                }
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => window.set_should_close(true),
                WindowEvent::Size(_width, _height) => {}
                WindowEvent::FramebufferSize(width, height) => {
                    if width == 0 && height == 0 {
                        'minimized_waiting: loop {
                            glfw_handle.wait_events();
                            for (_, waitingevent) in glfw::flush_messages(&events) {
                                dbg!(waitingevent.clone());
                                match waitingevent {
                                    WindowEvent::FramebufferSize(width, height) => {
                                        if width == 0 && height == 0 {
                                            continue;
                                        }
                                        break 'minimized_waiting;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                WindowEvent::Close => {
                    break;
                }
                _ => {}
            }
        }

        _vk_ctx.last_frame_instant = time::Instant::now();
    }
    print!("Exited loop");
    cleanup_vulkan(_vk_ctx);
}

fn create_sync_primitives(device: &ash::Device) -> SyncPrimitives {
    let sem_create_info = vk::SemaphoreCreateInfo::default();
    let fence_create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED); // So we can wait on the fence at first frame without blocking
    // https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html
    let image_available;
    let frame_in_flight;
    unsafe {
        image_available = device.create_semaphore(&sem_create_info, None).unwrap();
        frame_in_flight = device.create_fence(&fence_create_info, None).unwrap();
    }

    return SyncPrimitives {
        image_available,
        frame_in_flight,
    };
}

fn destroy_sync_primitives(
    device: &ash::Device,
    sync_primitives: &[SyncPrimitives; MAX_FRAMES_IN_FLIGHT],
) {
    unsafe {
        for sync_primitive in sync_primitives {
            device.destroy_semaphore(sync_primitive.image_available, None);
            device.destroy_fence(sync_primitive.frame_in_flight, None);
        }
    }
}

/// Only useful for debugging one-off draws. Has no synchronization.
#[deprecated]
fn draw_frame_once(vk_ctx: &VulkanContext) {
    unsafe {
        // 1. Get next swapchain image
        let (image_index, _) = vk_ctx
            .swapchain
            .swapchain_loader
            .acquire_next_image(
                vk_ctx.swapchain.swapchain,
                u64::MAX,              // No timeout
                vk::Semaphore::null(), // No semaphore (unsafe!)
                vk::Fence::null(),     // No fence (unsafe!)
            )
            .expect("Failed to acquire swapchain image");

        // 2. Record and submit your draw commands
        let cmd_buffer = vk_ctx.swapchain.command_buffers[0];

        // Your draw code here (reset command buffer first!)

        record_command_buffer(&vk_ctx, image_index, 0);

        // 3. Submit to GPU
        let command_buffers = [cmd_buffer];
        let submit_info: vk::SubmitInfo<'_> =
            vk::SubmitInfo::default().command_buffers(&command_buffers);
        // TODO semaphore to wait for COLOR_ATTACHMENT_OUTPUT pipeline stage.
        let submits = [submit_info];

        vk_ctx
            .device
            .queue_submit(vk_ctx.queue, &submits, vk::Fence::null())
            .expect("Failed to submit draw command buffer");

        // 4. Present the image
        let swapchains = [vk_ctx.swapchain.swapchain];
        let indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&indices);

        vk_ctx
            .swapchain
            .swapchain_loader
            .queue_present(vk_ctx.queue, &present_info)
            .expect("Failed to present");

        // 5. Wait for everything to finish (brute force sync)
        vk_ctx
            .device
            .device_wait_idle()
            .expect("Failed to wait for device idle");
    }
}

/// More efficient draw command without blocking for whole-device idle.
fn draw_frame_by_index(vk_ctx: &VulkanContext, frameidx: usize) -> Result<(), vk::Result> {
    unsafe {
        vk_ctx
            .device
            .wait_for_fences(
                &[vk_ctx.sync_primitives[frameidx].frame_in_flight],
                true,
                u64::MAX,
            )
            .expect("Failed to wait for the fence");
        // 1. Get next swapchain image
        vk_ctx
            .device
            .reset_fences(&[vk_ctx.sync_primitives[frameidx].frame_in_flight])
            .expect("Failed to reset fence");

        let (image_index, _) = vk_ctx.swapchain.swapchain_loader.acquire_next_image(
            vk_ctx.swapchain.swapchain,
            u64::MAX, // No timeout
            vk_ctx.sync_primitives[frameidx].image_available,
            vk::Fence::null(), // No fence
        )?;

        // 2. Record and submit your draw commands
        let cmd_buffer = vk_ctx.swapchain.command_buffers[frameidx];
        record_command_buffer(&vk_ctx, image_index, frameidx);
        update_uniform_buffer(&vk_ctx, frameidx);

        // 3. Submit to GPU
        let command_buffers = [cmd_buffer];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let queue_submit_wait_semaphores = [vk_ctx.sync_primitives[frameidx].image_available];
        // See https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html
        // The submit finishing does not imply image presentation finished, so Vulkan cannot guarantee the semaphore is not still in use unless either:
        // 1. an extension is used to give image_present a completion sync (lame), or
        // 2. you ensure you don't reuse a semaphore until the image is next available (which does ensure the semaphore is free)
        // I choose 2, so queue_submit signal semaphores must be indexed by swapch image
        let queue_submit_signal_semaphores =
            [vk_ctx.swapchain.swapchain_images[image_index as usize].render_finished];
        let submit_info: vk::SubmitInfo<'_> = vk::SubmitInfo::default()
            .command_buffers(&command_buffers)
            .wait_dst_stage_mask(&wait_stages)
            .wait_semaphores(&queue_submit_wait_semaphores)
            .signal_semaphores(&queue_submit_signal_semaphores);
        let submits = [submit_info];

        vk_ctx
            .device
            .queue_submit(
                vk_ctx.queue,
                &submits,
                vk_ctx.sync_primitives[frameidx].frame_in_flight,
            )
            .expect("Failed to submit draw command buffer");

        // 4. Present the image
        let swapchains = [vk_ctx.swapchain.swapchain];
        let indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&indices)
            .wait_semaphores(&queue_submit_signal_semaphores);

        vk_ctx
            .swapchain
            .swapchain_loader
            .queue_present(vk_ctx.queue, &present_info)?;
    }

    return Ok(());
}

fn cleanup_vulkan(vk_ctx: &mut VulkanContext) {
    dbg!("Cleanup");
    unsafe {
        vk_ctx
            .device
            .device_wait_idle()
            .expect("Couldn't wait for idle device for cleanup");

        destroy_swapchain_system(vk_ctx);
        destroy_sync_primitives(&vk_ctx.device, &vk_ctx.sync_primitives);

        vk_ctx
            .device
            .destroy_buffer(vk_ctx.bufs.devloc_vertex, None);
        vk_ctx.device.destroy_buffer(vk_ctx.bufs.devloc_index, None);
        vk_ctx.device.free_memory(vk_ctx.bufs.index_mem, None);
        vk_ctx.device.free_memory(vk_ctx.bufs.vertex_mem, None);
        for i in 0..vk_ctx.bufs.unibufs.len() {
            UniformBufferObject::destroy_uniform_buffer(&vk_ctx.device, &vk_ctx.bufs.unibufs[i]);
        }

        for &shader in &vk_ctx.pipeline_system.shader_mod {
            vk_ctx.device.destroy_shader_module(shader, None);
        }
        vk_ctx
            .device
            .destroy_pipeline(vk_ctx.pipeline_system.graphics_pipeline, None);
        vk_ctx
            .device
            .destroy_pipeline_layout(vk_ctx.pipeline_system.pipeline_layout, None);
        vk_ctx.device.destroy_render_pass(vk_ctx.render_pass, None);
        vk_ctx
            .device
            .destroy_descriptor_pool(vk_ctx.bufs.descriptor_pool, None);
        vk_ctx
            .device
            .destroy_descriptor_set_layout(vk_ctx.bufs.descriptor_set_layout, None);

        vk_ctx
            .debug_loader
            .destroy_debug_utils_messenger(vk_ctx.debug_msg_handler, None);

        vk_ctx.device.destroy_device(None);
        vk_ctx.surface_loader.destroy_surface(vk_ctx.surface, None);
        vk_ctx.instance.destroy_instance(None);
    }
}

fn shader_module_from_bytes(device: &ash::Device, bytes: &[u8]) -> vk::ShaderModule {
    let shader_code = ash::util::read_spv(&mut std::io::Cursor::new(bytes)).unwrap();
    let create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);

    unsafe {
        return device.create_shader_module(&create_info, None).unwrap();
    }
}

fn create_render_pass(device: &ash::Device, swapchain: &Swapchain) -> vk::RenderPass {
    let color_attachment: AttachmentDescription = vk::AttachmentDescription::default()
        .format(swapchain.swapchain_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
    let depth_stencil_attachment: AttachmentDescription = vk::AttachmentDescription::default()
        .format(DepthBufferSystem::format())
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let color_attachment_ref = vk::AttachmentReference::default()
        .attachment(0) // the index of 'color_attachment' in 'attachments'array below
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let depth_stencil_attachment_ref = vk::AttachmentReference::default()
        .attachment(1) // the index of 'depth_stencil_attachment'
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let color_attachments_references = [color_attachment_ref];
    // Only makes sense to have <= 1 depth_sences_attachment_reference per render pass

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachments_references)
        .depth_stencil_attachment(&depth_stencil_attachment_ref);

    let dependencies = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );

    let attachments = [color_attachment, depth_stencil_attachment];
    let subpasses: [vk::SubpassDescription<'_>; 1] = [subpass];
    let subpass_dependencies = [dependencies];
    let render_pass_create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&subpass_dependencies);

    let render_pass: vk::RenderPass;
    unsafe {
        render_pass = device
            .create_render_pass(&render_pass_create_info, None)
            .unwrap();
    }

    render_pass
}

fn create_descriptor_sets_in_new_pool(
    device: &ash::Device,
    layout: vk::DescriptorSetLayout,
) -> (vk::DescriptorPool, Vec<vk::DescriptorSet>) {
    let dpsize = vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(MAX_FRAMES_IN_FLIGHT as u32);
    let pool_sizes = [dpsize];
    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&pool_sizes)
        .max_sets(MAX_FRAMES_IN_FLIGHT as u32)
        .flags(vk::DescriptorPoolCreateFlags::empty());

    let descriptor_pool: vk::DescriptorPool;
    unsafe {
        descriptor_pool = device
            .create_descriptor_pool(&pool_create_info, None)
            .unwrap();
    }

    let layouts: [DescriptorSetLayout; MAX_FRAMES_IN_FLIGHT] = [layout; MAX_FRAMES_IN_FLIGHT];
    let set_alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&layouts);
    let descriptor_sets: Vec<vk::DescriptorSet>;
    unsafe {
        descriptor_sets = device.allocate_descriptor_sets(&set_alloc_info).unwrap();
    }

    return (descriptor_pool, descriptor_sets);
}

fn create_framebuffers(
    device: &ash::Device,
    swapchain: &Swapchain,
    render_pass: vk::RenderPass,
    depth_stencil_buffers: &Vec<DepthBufferSystem>,
) -> Vec<vk::Framebuffer> {
    swapchain
        .swapchain_images
        .iter()
        .zip(depth_stencil_buffers.iter())
        .map(|(image, depth_stencil_buffer)| {
            let attachments = [image.view, depth_stencil_buffer.depth_image_view];

            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass) // The lifetime of the underlyng render pass referenced by the RenderPass numeric handle should be owned by VulkanContext. so there is an implied &'a' here for the lifetime of the latent RenderPass in the driver.
                .attachments(&attachments)
                .width(swapchain.swapchain_extent.width)
                .height(swapchain.swapchain_extent.height)
                .layers(1);

            unsafe {
                device
                    .create_framebuffer(&framebuffer_info, None)
                    .expect("Failed to create framebuffer")
            }
        })
        .collect()
}

fn create_graphics_pipeline(
    device: &ash::Device,
    swapchain: &Swapchain,
    render_pass: vk::RenderPass,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> GraphicsPipeline {
    // no shader code constants yet
    let specialization_info = vk::SpecializationInfo::default();

    // Vertex Shader setup
    let vert_shader_mod = shader_module_from_bytes(&device, crate::shader::VERT);
    let vert_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vert_shader_mod)
        .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
        .specialization_info(&specialization_info);

    // Frag Shader setup
    let frag_shader_mod = shader_module_from_bytes(&device, crate::shader::FRAG);
    let frag_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(frag_shader_mod)
        .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
        .specialization_info(&specialization_info);

    let shader_stages = [vert_create_info, frag_create_info];

    let dynamic_states: [vk::DynamicState; 2] =
        [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state_create_info =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let vertex_binding_descriptions = [Vertex::get_binding_description()];
    let vertex_attribute_descriptions = Vertex::get_attribute_descriptions();
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
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
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

    let depthstencil_create_info = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS)
        .depth_bounds_test_enable(false)
        .min_depth_bounds(0.0) // Disabled if depth_bounds_test disabled
        .max_depth_bounds(1.0) // Disabled if depth_bounds_test disabled
        .stencil_test_enable(false);
    // .front, .back disabled

    // Since we have just one color attachment ref in our render pass (for one color attachment ImageView in any framebuffer), we have only one blend attachment
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

    let set_layouts = [descriptor_set_layout];
    let push_constant_ranges = [];
    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&set_layouts)
        .push_constant_ranges(&push_constant_ranges);

    let pipeline_layout;
    unsafe {
        pipeline_layout = device
            .create_pipeline_layout(&pipeline_layout_create_info, None)
            .unwrap();
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
        graphics_pipeline = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
            .unwrap();
    }

    if !(graphics_pipeline.len() == 1) {
        panic!("I thought there would be exactly one graphics pipeline...");
    }

    return GraphicsPipeline {
        graphics_pipeline: graphics_pipeline[0],
        pipeline_layout: pipeline_layout,
        shader_mod: vec![vert_shader_mod, frag_shader_mod],
    };
}

fn record_command_buffer(vk_ctx: &VulkanContext, image_index: u32, frame_index: usize) {
    let inheritance_info: vk::CommandBufferInheritanceInfo<'_> =
        vk::CommandBufferInheritanceInfo::default();
    let cmd_buffer_begin_info = vk::CommandBufferBeginInfo::default()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
        .inheritance_info(&inheritance_info);
    let cmd_buffer_target = vk_ctx.swapchain.command_buffers[frame_index];
    unsafe {
        vk_ctx
            .device
            .reset_command_buffer(cmd_buffer_target, vk::CommandBufferResetFlags::empty())
            .expect("Failed to reset command buffer");
        vk_ctx
            .device
            .begin_command_buffer(cmd_buffer_target, &cmd_buffer_begin_info)
            .expect("Failed to start recording for command buffer");
    }

    let clear_values = [
        vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        },
        vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        },
    ];
    let render_pass_begin_info = vk::RenderPassBeginInfo::default()
        .render_pass(vk_ctx.render_pass) //implicitly borrowed
        .framebuffer(vk_ctx.framebuffers[image_index as usize]) //TODO eliminate needless recasting
        .render_area(
            vk::Rect2D::default()
                .offset(vk::Offset2D { x: 0, y: 0 })
                .extent(vk_ctx.swapchain.swapchain_extent),
        )
        .clear_values(&clear_values);

    // We already set these up in create_graphics_pipeline, but we reinclude them here since we set the pipeline to set these dynamically.
    let viewport = vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(vk_ctx.swapchain.swapchain_extent.width as f32)
        .height(vk_ctx.swapchain.swapchain_extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0);
    let scissor = vk::Rect2D::default()
        .offset(vk::Offset2D::default())
        .extent(vk_ctx.swapchain.swapchain_extent);
    let viewports = [viewport];
    let scissors = [scissor];

    let vertex_buffers: [vk::Buffer; 1] = [vk_ctx.bufs.devloc_vertex];
    let indexdata = triangle_geom_indices();
    let offsets = [0];
    let descriptor_sets = [vk_ctx.bufs.unibufs[frame_index].desc_set];
    let dynamic_offsets = [];
    unsafe {
        vk_ctx.device.cmd_begin_render_pass(
            cmd_buffer_target,
            &render_pass_begin_info,
            vk::SubpassContents::INLINE,
        );
        vk_ctx.device.cmd_bind_pipeline(
            cmd_buffer_target,
            vk::PipelineBindPoint::GRAPHICS,
            vk_ctx.pipeline_system.graphics_pipeline,
        );
        vk_ctx
            .device
            .cmd_set_viewport(cmd_buffer_target, 0, &viewports);
        vk_ctx
            .device
            .cmd_set_scissor(cmd_buffer_target, 0, &scissors);

        // Draw this
        vk_ctx
            .device
            .cmd_bind_vertex_buffers(cmd_buffer_target, 0, &vertex_buffers, &offsets);
        vk_ctx.device.cmd_bind_index_buffer(
            cmd_buffer_target,
            vk_ctx.bufs.devloc_index,
            0,
            vk::IndexType::UINT16,
        ); // should match GeometryDataIndex
        vk_ctx.device.cmd_bind_descriptor_sets(
            cmd_buffer_target,
            vk::PipelineBindPoint::GRAPHICS,
            vk_ctx.pipeline_system.pipeline_layout,
            0,
            &descriptor_sets,
            &dynamic_offsets,
        );
        vk_ctx
            .device
            .cmd_draw_indexed(cmd_buffer_target, indexdata.len() as u32, 1, 0, 0, 0);

        vk_ctx.device.cmd_end_render_pass(cmd_buffer_target);
        vk_ctx
            .device
            .end_command_buffer(cmd_buffer_target)
            .expect("An error occured while drawing");
    }
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

    glfw::init_hint(glfw::InitHint::Platform(platform));
    let mut glfwh = glfw::init(fail_on_errors!()).unwrap();
    let (mut window, events) = create_window(&mut glfwh);
    let mut vk_ctx = init_vulkan(&glfwh, &mut window);

    event_loop(&mut glfwh, &mut window, events, &mut vk_ctx);
}

// Geometry

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
    // Could add texture coords, ambient occlusion, etc.
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl Vertex {
    fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
    }
    fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        let first = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, position) as u32);
        let second = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, color) as u32);

        return [first, second];
    }
}

fn get_triangle_geometry() -> Vec<Vertex> {
    vec![
        Vertex {
            position: [0.0, -0.5, 0.0],
            color: [1.0, 0.0, 0.2],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            color: [0.1, 0.5, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.0],
            color: [0.7, 0.5, 0.0],
        },
    ]
}

fn get_triangle_geometry2() -> Vec<Vertex> {
    vec![
        Vertex {
            position: [0.5, -0.5, 0.0],
            color: [1.0, 0.0, 0.2],
        },
        Vertex {
            position: [0.0, 0.5, 0.0],
            color: [0.1, 0.5, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            color: [0.7, 0.5, 0.0],
        },
    ]
}

type GeometryDataIndex = u16; // Can be u16 or u32 

fn triangle_vertices_indexed() -> Vec<Vertex> {
    vec![
        Vertex {
            position: [-0.5, -0.5, 0.0],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.0],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            color: [0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.0],
            color: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [-0.5, -0.5, -0.5],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            color: [0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            color: [1.0, 1.0, 1.0],
        },
    ]
}

fn triangle_geom_indices() -> Vec<GeometryDataIndex> {
    vec![0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4]
}

/// Obtain an allocate info via memory requirements (see get_*_memory_requirements function for a resource, * = buffer, image, etc)
fn reconcile_memory_requirements_with_physical_device_memory_types<'a>(
    mem_requirements: vk::MemoryRequirements,
    instance: &ash::Instance,
    desired_properties: vk::MemoryPropertyFlags,
    physical_device: vk::PhysicalDevice,
) -> vk::MemoryAllocateInfo<'a> {
    let mem_properties: vk::PhysicalDeviceMemoryProperties;
    unsafe {
        mem_properties = instance.get_physical_device_memory_properties(physical_device);
    }

    let mut memory_type_index: u32 = 0;
    // https://docs.vulkan.org/refpages/latest/refpages/source/VkMemoryRequirements.html
    // memoryTypeBits is a bitmask and contains one bit set for every supported memory type for the resource. Bit i is set if and only if the memory type i in the VkPhysicalDeviceMemoryProperties structure for the physical device is supported for the resource.
    while memory_type_index < mem_properties.memory_type_count {
        if (mem_requirements.memory_type_bits & (1 << memory_type_index) > 0)
            && (mem_properties.memory_types[memory_type_index as usize].property_flags
                & desired_properties
                == desired_properties)
        {
            break;
        }
        memory_type_index += 1;
    }

    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_requirements.size)
        .memory_type_index(memory_type_index);

    return allocate_info;
}

fn create_and_allocate_buffer(
    device: &ash::Device,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    buffer_create_info: vk::BufferCreateInfo,
    desired_properties: vk::MemoryPropertyFlags,
) -> (vk::Buffer, vk::DeviceMemory) {
    let buffer: vk::Buffer;
    let mem_requirements: vk::MemoryRequirements;
    unsafe {
        buffer = device
            .create_buffer(&buffer_create_info, None)
            .expect("Unable to create vertex buffer");
        mem_requirements = device.get_buffer_memory_requirements(buffer);
    }

    let allocate_info = reconcile_memory_requirements_with_physical_device_memory_types(
        mem_requirements,
        instance,
        desired_properties,
        physical_device,
    );
    let device_memory: vk::DeviceMemory;
    unsafe {
        device_memory = device.allocate_memory(&allocate_info, None).unwrap();
        // For many objects, you are supposed to bind at different offests to the same DeviceMemory used as a pool. its better to let a library like gpu_allocator manage the DeviceMemory and offsets.
        device.bind_buffer_memory(buffer, device_memory, 0).unwrap();
    }

    return (buffer, device_memory);
}

fn create_and_fill_hostvis_vertex_buffer(
    device: &ash::Device,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    vertices: &[Vertex],
) -> (vk::Buffer, vk::DeviceMemory) {
    let create_info = vk::BufferCreateInfo::default()
        .size((vertices.len() * size_of::<Vertex>()) as u64)
        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .flags(vk::BufferCreateFlags::empty());
    let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

    let (buffer, device_memory) =
        create_and_allocate_buffer(device, instance, physical_device, create_info, props);

    fill_buffer_via_host_mapping(device, device_memory, vertices);
    return (buffer, device_memory);
}

fn create_and_fill_hostvis_staging_buffer<T>(
    device: &ash::Device,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    data: &[T],
) -> (vk::Buffer, vk::DeviceMemory)
where
    T: Copy,
{
    let create_info = vk::BufferCreateInfo::default()
        .size(std::mem::size_of_val(data) as u64)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .flags(vk::BufferCreateFlags::empty());
    let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

    let (buffer, device_memory) =
        create_and_allocate_buffer(device, instance, physical_device, create_info, props);

    fill_buffer_via_host_mapping(device, device_memory, data);
    return (buffer, device_memory);
}

fn create_device_local_vertex_buffer(
    device: &ash::Device,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    vertices: &[Vertex],
) -> (vk::Buffer, vk::DeviceMemory) {
    let create_info = vk::BufferCreateInfo::default()
        .size(std::mem::size_of_val(vertices) as u64)
        .usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .flags(vk::BufferCreateFlags::empty());
    let props = vk::MemoryPropertyFlags::DEVICE_LOCAL;

    let (buffer, device_memory) =
        create_and_allocate_buffer(device, instance, physical_device, create_info, props);

    return (buffer, device_memory);
}

fn create_device_local_index_buffer(
    device: &ash::Device,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    indices: &[GeometryDataIndex],
) -> (vk::Buffer, vk::DeviceMemory) {
    let create_info = vk::BufferCreateInfo::default()
        .size(std::mem::size_of_val(indices) as u64)
        .usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .flags(vk::BufferCreateFlags::empty());
    let props = vk::MemoryPropertyFlags::DEVICE_LOCAL;

    let (buffer, device_memory) =
        create_and_allocate_buffer(device, instance, physical_device, create_info, props);

    return (buffer, device_memory);
}

fn transfer_buffers_on_device(
    vk_ctx: &VulkanContext,
    src_buffer: vk::Buffer,
    dst_buffer: vk::Buffer,
    size: vk::DeviceSize,
) {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_pool(vk_ctx.swapchain.command_resources)
        .command_buffer_count(1);

    let cmd_buffer: Vec<vk::CommandBuffer>;
    unsafe {
        cmd_buffer = vk_ctx.device.allocate_command_buffers(&alloc_info).unwrap();
    }
    if cmd_buffer.len() != 1 {
        panic!("Wrong # cmd buffers allocated");
    }
    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    let bufcopy = vk::BufferCopy::default()
        .src_offset(0)
        .dst_offset(0)
        .size(size);
    let submit_info = [vk::SubmitInfo::default().command_buffers(&cmd_buffer)];

    unsafe {
        vk_ctx
            .device
            .begin_command_buffer(cmd_buffer[0], &begin_info)
            .unwrap();
        vk_ctx
            .device
            .cmd_copy_buffer(cmd_buffer[0], src_buffer, dst_buffer, &[bufcopy]);
        vk_ctx.device.end_command_buffer(cmd_buffer[0]).unwrap();
        // TODO can use a separate queue for this transfer, if additional concurrency is desired
        vk_ctx
            .device
            .queue_submit(vk_ctx.queue, &submit_info, vk::Fence::null())
            .unwrap();
        vk_ctx.device.queue_wait_idle(vk_ctx.queue).unwrap();
        vk_ctx
            .device
            .free_command_buffers(vk_ctx.swapchain.command_resources, &cmd_buffer);
    }
}

fn fill_buffer_via_host_mapping<T>(device: &ash::Device, memory: vk::DeviceMemory, data: &[T])
where
    T: Copy,
{
    unsafe {
        let memptr: *mut std::ffi::c_void = device
            .map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
            .unwrap();
        let mapped_slice = std::slice::from_raw_parts_mut(memptr as *mut T, data.len());
        mapped_slice.copy_from_slice(data);
        device.unmap_memory(memory);
    }
}

fn load_vertex_data_via_staging_buffer(vk_ctx: &VulkanContext, vertex_data: &[Vertex]) {
    let (stg_buf, stg_mem) = create_and_fill_hostvis_staging_buffer(
        &vk_ctx.device,
        &vk_ctx.instance,
        vk_ctx.physical_device,
        vertex_data,
    );

    transfer_buffers_on_device(
        vk_ctx,
        stg_buf,
        vk_ctx.bufs.devloc_vertex,
        std::mem::size_of_val(vertex_data) as u64,
    );

    unsafe {
        vk_ctx.device.destroy_buffer(stg_buf, None);
        vk_ctx.device.free_memory(stg_mem, None);
    }
}
fn load_index_data_via_staging_buffer(vk_ctx: &VulkanContext, index_data: &[GeometryDataIndex]) {
    let (stg_buf, stg_mem) = create_and_fill_hostvis_staging_buffer(
        &vk_ctx.device,
        &vk_ctx.instance,
        vk_ctx.physical_device,
        index_data,
    );

    transfer_buffers_on_device(
        vk_ctx,
        stg_buf,
        vk_ctx.bufs.devloc_index,
        std::mem::size_of_val(index_data) as u64,
    );

    unsafe {
        vk_ctx.device.destroy_buffer(stg_buf, None);
        vk_ctx.device.free_memory(stg_mem, None);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct UniformBufferObject {
    model: glam::Mat4,
    view: glam::Mat4, // 64 bytes
    proj: glam::Mat4, // 64 bytes
}

impl UniformBufferObject {
    fn uniform_buffer(
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        descriptor_set: vk::DescriptorSet,
    ) -> UniformBufSubsys {
        let create_info = vk::BufferCreateInfo::default()
            .size(std::mem::size_of::<UniformBufferObject>() as u64)
            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

        let (buf, devmem) =
            create_and_allocate_buffer(device, instance, physical_device, create_info, props);
        let memptr: *mut std::ffi::c_void;
        unsafe {
            memptr = device
                .map_memory(devmem, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                .unwrap();
        }

        let buffer_info = vk::DescriptorBufferInfo::default()
            .buffer(buf)
            .offset(0)
            .range(std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize);

        let descriptor_write = vk::WriteDescriptorSet::default()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(std::slice::from_ref(&buffer_info));

        unsafe {
            device.update_descriptor_sets(&[descriptor_write], &[]);
        }

        return UniformBufSubsys {
            uniform_buffer: buf,
            unif_mem: devmem,
            mapped: memptr as *mut UniformBufferObject,
            desc_set: descriptor_set,
        };
    }

    fn destroy_uniform_buffer(device: &ash::Device, uniform_buffer_subsystem: &UniformBufSubsys) {
        unsafe {
            device.destroy_buffer(uniform_buffer_subsystem.uniform_buffer, None);
            device.free_memory(uniform_buffer_subsystem.unif_mem, None);
        }
    }

    fn descriptor_set_layout(device: &ash::Device) -> DescriptorSetLayout {
        let _samplers: [vk::Sampler; 0] = [];
        let ubo_layout_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX);

        let bindings = [ubo_layout_binding];

        let ubo_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

        let desc_set_layout;
        unsafe {
            desc_set_layout = device
                .create_descriptor_set_layout(&ubo_layout_create_info, None)
                .unwrap();
        }

        return desc_set_layout;
    }
}

fn update_uniform_buffer(vk_ctx: &VulkanContext, frameidx: usize) {
    let _deltat = vk_ctx.last_frame_instant.elapsed().as_secs_f32();
    let _elapsedt = vk_ctx.program_start.elapsed().as_secs_f32();
    let mut unif: UniformBufferObject = UniformBufferObject {
        model: Mat4::from_rotation_z(_elapsedt * 90.0f32.to_radians()),
        view: Mat4::look_at_rh(
            Vec3::new(2.0, 2.0, 2.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        proj: Mat4::perspective_rh(
            45.0f32.to_radians(),
            vk_ctx.swapchain.swapchain_extent.width as f32
                / vk_ctx.swapchain.swapchain_extent.height as f32,
            0.1,
            10.0,
        ),
    };
    unif.proj.y_axis.y *= -1.0;
    unsafe {
        let mapped_slice = std::slice::from_raw_parts_mut(vk_ctx.bufs.unibufs[frameidx].mapped, 1);
        mapped_slice.copy_from_slice(&[unif]);
    }
}

// Depth Buffering

struct DepthBufferSystem {
    depth_image: vk::Image,
    depth_image_memory: vk::DeviceMemory,
    depth_image_view: vk::ImageView,
}

impl DepthBufferSystem {
    fn new(
        device: &ash::Device,
        instance: &ash::Instance,
        swapchain: &Swapchain,
        physical_device: vk::PhysicalDevice,
    ) -> Self {
        let dsformat = Self::format();

        let image_create_info = vk::ImageCreateInfo::default()
            .flags(vk::ImageCreateFlags::empty())
            .image_type(vk::ImageType::TYPE_2D)
            .format(dsformat)
            .extent(vk::Extent3D {
                width: swapchain.swapchain_extent.width,
                height: swapchain.swapchain_extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&[])
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image: vk::Image;
        unsafe {
            image = device.create_image(&image_create_info, None).unwrap();
        }

        let image_view_create_info: vk::ImageViewCreateInfo = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(dsformat)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );

        let mem_requirements: vk::MemoryRequirements;

        let desired_properties = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        let memory_allocate_info: MemoryAllocateInfo<'_>;
        let dev_mem: DeviceMemory;
        unsafe {
            mem_requirements = device.get_image_memory_requirements(image);
            memory_allocate_info = reconcile_memory_requirements_with_physical_device_memory_types(
                mem_requirements,
                instance,
                desired_properties,
                physical_device,
            );
            dev_mem = device.allocate_memory(&memory_allocate_info, None).unwrap();
            device.bind_image_memory(image, dev_mem, 0).unwrap();
        }

        let image_view: vk::ImageView;
        unsafe {
            image_view = device
                .create_image_view(&image_view_create_info, None)
                .unwrap();
        }

        Self {
            depth_image: image,
            depth_image_view: image_view,
            depth_image_memory: dev_mem,
        }
    }

    fn format() -> vk::Format {
        vk::Format::D32_SFLOAT //TODO query hardware for supported format
    }

    fn destroy(self: &Self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.depth_image_view, None);
            device.destroy_image(self.depth_image, None);
            device.free_memory(self.depth_image_memory, None);
        }
    }
}
