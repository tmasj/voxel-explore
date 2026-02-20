use crate::window::*;
use ash;
use ash::vk;
use std::ffi::{CStr, CString, c_char};

pub struct VulkanKernel {
    entry: ash::Entry,
    instance: ash::Instance,
}

impl VulkanKernel {
    pub fn new_vulkan_instance(
        entry: &ash::Entry,
        extension_names: &Vec<CString>,
    ) -> ash::Instance {
        let app_info = vk::ApplicationInfo::default()
            .application_name(CStr::from_bytes_with_nul(b"Voxel Explore\0").unwrap())
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(CStr::from_bytes_with_nul(b"No Engine\0").unwrap())
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_3);

        let layer_names: [&CStr; 1];
        unsafe {
            layer_names = [CStr::from_bytes_with_nul_unchecked(
                b"VK_LAYER_KHRONOS_validation\0",
            )];
        }
        let layer_names_raw: Vec<*const i8> =
            layer_names.iter().map(|name| name.as_ptr()).collect();

        let extension_names_ptr: Vec<*const c_char> =
            extension_names.iter().map(|s| s.as_ptr()).collect();

        // Enable printf debugging
        // https://github.com/KhronosGroup/Vulkan-Samples/blob/e6ada08f110de050636617a08821368efa7cd23b/samples/extensions/shader_debugprintf/README.adoc#L45
        let enabled_validation_features = [vk::ValidationFeatureEnableEXT::DEBUG_PRINTF];
        let mut pnext_validation_features = vk::ValidationFeaturesEXT::default()
            .enabled_validation_features(&enabled_validation_features);

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names_ptr)
            .enabled_layer_names(&layer_names_raw)
            .push_next(&mut pnext_validation_features);

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

    pub fn new_from_glfw(glfw_kernel: &GlfwKernel) -> Self {
        let entry = unsafe { ash::Entry::load().unwrap() };
        let mut extension_names = glfw_kernel.vulkan_extension_names();
        extension_names.push(CString::from(ash::ext::debug_utils::NAME));
        let instance = Self::new_vulkan_instance(&entry, &extension_names);

        return VulkanKernel { entry, instance };
    }

    pub fn pick_physical_device(
        self: &Self,
        surface_loader: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> (vk::PhysicalDevice, u32) {
        let physical_devices = unsafe { self.instance.enumerate_physical_devices().unwrap() };
        let physical_device = physical_devices[0];

        let queue_families = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(physical_device)
        };

        let queue_family_index = queue_families
            .iter()
            .enumerate()
            .find(|(i, qf)| {
                qf.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && unsafe {
                        surface_loader
                            .get_physical_device_surface_support(
                                physical_device,
                                *i as u32,
                                surface,
                            )
                            .unwrap()
                    }
            })
            .map(|(i, _)| i as u32)
            .expect("No suitable queue family");

        (physical_device, queue_family_index)
    }

    pub fn new_surface(
        self: &Self,
        windowing: &WindowLifecycle,
    ) -> (vk::SurfaceKHR, ash::khr::surface::Instance) {
        // TODO I believe the Instance's latent state in the driver is mutated by this pub fn, so that should be reflected in the argument if so.
        let mut surface_handle: glfw::ffi::VkSurfaceKHR = std::ptr::null_mut();
        let result: glfw::ffi::VkResult;
        unsafe {
            result = windowing.window.create_window_surface(
                self.instance.handle().as_raw() as *mut glfw::ffi::VkInstance_T,
                std::ptr::null(),
                &mut surface_handle,
            );
        }
        assert_eq!(result, vk::Result::SUCCESS.as_raw());

        let surface = vk::SurfaceKHR::from_raw(surface_handle as u64);
        let surface_loader = ash::khr::surface::Instance::new(&self.entry, &self.instance);

        return (surface, surface_loader);
    }

    pub fn new_logical_device_and_queue(
        self: &Self,
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
            self.instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap()
        };
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        return (device, queue);
    }

    pub fn new_debug_instance(
        self: &Self,
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
            debug_loader = ash::ext::debug_utils::Instance::new(&self.entry, &self.instance);
            messenger = debug_loader
                .create_debug_utils_messenger(&debug_messenger_info, None)
                .unwrap();
        }

        return (debug_loader, messenger);
    }
}

impl Drop for VulkanKernel {
    fn drop() {
        vk_ctx.instance.destroy_instance(None);
    }
}

pub struct VulkanDeviceContext {
    physical_device: vk::PhysicalDevice,
    device: ash::Device, //TODO decide between ash::Device and vk::Device
    debug_msg_handler: vk::DebugUtilsMessengerEXT,
    debug_loader: ash::ext::debug_utils::Instance,
    surface: vk::SurfaceKHR,
    surface_loader: ash::khr::surface::Instance,
    queue: vk::Queue,
    queue_family_index: u32,
    vulkan_kernel: Arc<VulkanKernel>,
}

impl VulkanDeviceContext {
    pub fn new(vulkan_kernel: Arc<VulkanKernel>, windowing: &WindowLifecycle) -> Self {
        let (debug_loader, debug_msg_handler) = vulkan_kernel.new_debug_instance();
        let (surface, surface_loader) = vulkan_kernel.new_surface(windowing);
        let (physical_device, queue_family_index) =
            vulkan_kernel.pick_physical_device(&surface_loader, surface);
        let (device, queue) =
            vulkan_kernel.new_logical_device_and_queue(physical_device, queue_family_index);
        Self {
            physical_device,
            device,
            debug_msg_handler,
            debug_loader,
            surface,
            surface_loader,
            queue,
            queue_family_index,
            vulkan_kernel,
        }
    }

    pub fn create_descriptor_sets_in_new_pool(
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

    pub fn shader_module_from_bytes(device: &ash::Device, bytes: &[u8]) -> vk::ShaderModule {
        let shader_code = ash::util::read_spv(&mut std::io::Cursor::new(bytes)).unwrap();
        let create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);

        unsafe {
            return device.create_shader_module(&create_info, None).unwrap();
        }
    }

    /// Obtain an allocate info via memory requirements (see get_*_memory_requirements function for a resource, * = buffer, image, etc)
    pub fn reconcile_memory_requirements_with_physical_device_memory_types<'a>(
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

    pub fn create_and_allocate_buffer(
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

    pub fn create_and_fill_hostvis_vertex_buffer(
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

    pub fn create_and_fill_hostvis_staging_buffer<T>(
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

    pub fn create_device_local_vertex_buffer(
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

    pub fn create_device_local_index_buffer(
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

    pub fn transfer_buffers_on_device(
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
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
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

    pub fn fill_buffer_via_host_mapping<T>(
        device: &ash::Device,
        memory: vk::DeviceMemory,
        data: &[T],
    ) where
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

    pub fn load_vertex_data_via_staging_buffer(vk_ctx: &VulkanContext, vertex_data: &[Vertex]) {
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
    pub fn load_index_data_via_staging_buffer(
        vk_ctx: &VulkanContext,
        index_data: &[GeometryDataIndex],
    ) {
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
}

impl Drop for VulkanDeviceContext {
    fn drop(self: &mut Self) {
        vk_ctx
            .debug_loader
            .destroy_debug_utils_messenger(vk_ctx.debug_msg_handler, None);

        vk_ctx.device.destroy_device(None);
        vk_ctx.surface_loader.destroy_surface(vk_ctx.surface, None);
    }
}
