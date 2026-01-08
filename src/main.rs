use ash::vk::Handle;
use ash::{vk, Entry};
use glfw::{Action, Context, Glfw, Key, PWindow, WindowEvent, WindowHint, GlfwReceiver};
use glfw::fail_on_errors;
use std::ffi::{CStr, c_char};

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
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
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
) -> (ash::khr::swapchain::Device, vk::SwapchainKHR) {
    let surface_caps = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(physical_device, surface)
            .unwrap()
    };
    let surface_format = unsafe {
        surface_loader
            .get_physical_device_surface_formats(physical_device, surface)
            .unwrap()[0]
    };

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(surface_caps.min_image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(surface_caps.current_extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(surface_caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(vk::PresentModeKHR::FIFO);

    let swapchain_loader = ash::khr::swapchain::Device::new(instance,device);
    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&swapchain_create_info, None)
            .unwrap()
    };

    (swapchain_loader, swapchain)
}

fn init_vulkan(glfw_handle: &Glfw, window: &mut PWindow) -> VulkanContext {
    let entry = unsafe { Entry::load().unwrap() };
    let instance = create_vulkan_instance(glfw_handle, &entry);
    let (surface, surface_loader) = create_surface(window, &entry, &instance);
    let (physical_device, queue_family_index) =
        pick_physical_device(&instance, &surface_loader, surface);
    let (device, queue) = create_logical_device(&instance, physical_device, queue_family_index);
    let (swapchain_loader, swapchain) = create_swapchain(
        &instance,
        &device,
        physical_device,
        &surface_loader,
        surface,
    );

    VulkanContext {
        _entry: entry,
        instance,
        surface,
        surface_loader,
        physical_device,
        device,
        queue,
        queue_family_index,
        swapchain_loader,
        swapchain,
    }
}

fn event_loop(
    glfw_handle: &mut Glfw,
    window: &mut PWindow,
    events: Events,
    _vk_ctx: &VulkanContext,
) {
    let mut last_size = window.get_size();
    while !window.should_close() {
        glfw_handle.poll_events();
        let current_size = window.get_size();
        if current_size != last_size {
            println!("Window resized: {:?}", current_size);
            // handle resize here
            last_size = current_size;
        }
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Key(Key::W, _, Action::Press, _) => {
                    println!("W!");
                }
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true)
                }
                WindowEvent::Size(width, height) => {
                    println!("Resize: {}x{}", width, height);
                }
                WindowEvent::FramebufferSize(width, height) => {
                    println!("Framebuffer: {}x{}", width, height);
                }
                _ => {}
            }
        }
    }
}

fn cleanup_vulkan(vk_ctx: VulkanContext) {
    unsafe {
        vk_ctx
            .swapchain_loader
            .destroy_swapchain(vk_ctx.swapchain, None);
        vk_ctx.device.destroy_device(None);
        vk_ctx.surface_loader.destroy_surface(vk_ctx.surface, None);
        vk_ctx.instance.destroy_instance(None);
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

    glfw::init_hint(
        glfw::InitHint::Platform(platform)
    );
    let mut glfwh = glfw::init(fail_on_errors!()).unwrap();
    let (mut window, events) = create_window(&mut glfwh);
    let vk_ctx = init_vulkan(&glfwh, &mut window);

    event_loop(&mut glfwh, &mut window, events, &vk_ctx);

    cleanup_vulkan(vk_ctx);
}