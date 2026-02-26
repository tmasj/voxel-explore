use crate::window::*;
use ash;
use ash::vk;
use std::ffi::{CStr, CString, c_char};
use std::marker::PhantomData;
use std::ops::{Deref, Index};
use std::sync::Arc;

pub struct VulkanKernel {
    entry: ash::Entry,
    pub instance: ash::Instance,
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

        // TODO refactor to separate method
        let queue_families = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(physical_device)
        };

        // TODO support present/graphics as discrete queues. This only support graphics/present as one queue (surface_support result returns whether the queue supports presentation). Not always avail on some mobile graphics.
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
                vk::Handle::as_raw(self.instance.handle()) as *mut glfw::ffi::VkInstance_T,
                std::ptr::null(),
                &mut surface_handle,
            );
        }
        assert_eq!(result, vk::Result::SUCCESS.as_raw());

        let surface: vk::SurfaceKHR = vk::Handle::from_raw(surface_handle as u64);
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
    fn drop(self: &mut Self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

pub struct VulkanDeviceContext {
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device, //TODO decide between ash::Device and vk::Device
    debug_msg_handler: vk::DebugUtilsMessengerEXT,
    debug_loader: ash::ext::debug_utils::Instance,
    pub surface: vk::SurfaceKHR,
    pub surface_loader: ash::khr::surface::Instance,
    pub queue: vk::Queue,
    pub queue_family_index: u32,
    pub vulkan_kernel: Arc<VulkanKernel>,
}

impl Deref for VulkanDeviceContext {
    type Target = ash::Device;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
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

    pub fn shader_module_from_bytes(self: &Self, bytes: &[u8]) -> vk::ShaderModule {
        let shader_code = ash::util::read_spv(&mut std::io::Cursor::new(bytes)).unwrap();
        let create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);

        unsafe {
            return self
                .device
                .create_shader_module(&create_info, None)
                .unwrap();
        }
    }

    /// Obtain an allocate info via memory requirements (see get_*_memory_requirements function for a resource, * = buffer, image, etc)
    pub fn reconcile_memory_requirements_with_physical_device_memory_types<'a>(
        self: &Self,
        mem_requirements_source: impl HasMemReqs,
        desired_properties: vk::MemoryPropertyFlags,
    ) -> vk::MemoryAllocateInfo<'a> {
        let reqs = mem_requirements_source.mem_requirements(self);
        let mem_properties: vk::PhysicalDeviceMemoryProperties;
        unsafe {
            mem_properties = self
                .vulkan_kernel
                .instance
                .get_physical_device_memory_properties(self.physical_device);
        }

        let mut memory_type_index: u32 = 0;
        // https://docs.vulkan.org/refpages/latest/refpages/source/VkMemoryRequirements.html
        // memoryTypeBits is a bitmask and contains one bit set for every supported memory type for the resource. Bit i is set if and only if the memory type i in the VkPhysicalDeviceMemoryProperties structure for the physical device is supported for the resource.
        while memory_type_index < mem_properties.memory_type_count {
            if (reqs.memory_type_bits & (1 << memory_type_index) > 0)
                && (mem_properties.memory_types[memory_type_index as usize].property_flags
                    & desired_properties
                    == desired_properties)
            {
                break;
            }
            memory_type_index += 1;
        }

        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(reqs.size)
            .memory_type_index(memory_type_index);

        return allocate_info;
    }
}

impl Drop for VulkanDeviceContext {
    fn drop(self: &mut Self) {
        unsafe {
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_loader
                .destroy_debug_utils_messenger(self.debug_msg_handler, None);
        }
    }
}

#[derive(Default)]
pub struct DataManifest {
    len: u32,
}

impl DataManifest {
    pub fn len(self: &Self) -> u32 {
        self.len
    }

    pub fn offset(self: &Self) -> i32 {
        0
    }

    pub fn offset_unsigned(self: &Self) -> u32 {
        0 // but panic if ever negative
    }
}

pub struct AllocatedDeviceBuffer<T: Copy> {
    pub dev: Arc<VulkanDeviceContext>,
    pub buffer: vk::Buffer,
    pub mem: vk::DeviceMemory,
    pub manifest: DataManifest,
    map: *mut T,
    _phantom: PhantomData<T>,
}

impl<T: Copy> AllocatedDeviceBuffer<T> {
    pub fn new(
        device: &Arc<VulkanDeviceContext>,
        buffer_create_info: vk::BufferCreateInfo,
        desired_properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let buffer: vk::Buffer;
        unsafe {
            buffer = device
                .create_buffer(&buffer_create_info, None)
                .expect("Unable to create vertex buffer");
        }

        let allocate_info = device.reconcile_memory_requirements_with_physical_device_memory_types(
            buffer,
            desired_properties,
        );
        let mem: vk::DeviceMemory;
        unsafe {
            mem = device.allocate_memory(&allocate_info, None).unwrap();
            // For many objects, you are supposed to bind at different offests to the same DeviceMemory used as a pool. its better to let a library like gpu_allocator manage the DeviceMemory and offsets.
            device.bind_buffer_memory(buffer, mem, 0).unwrap();
        }

        let mut map: *mut std::ffi::c_void = Default::default();
        if desired_properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
            unsafe {
                map = device
                    .map_memory(mem, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                    .unwrap();
            }
        }

        return Self {
            dev: Arc::clone(device),
            buffer,
            mem,
            manifest: DataManifest::default(),
            map: map as *mut T,
            _phantom: PhantomData::default(),
        };
    }

    pub fn object_size() -> vk::DeviceSize {
        std::mem::size_of::<T>() as vk::DeviceSize
    }

    pub fn data_size(data: &[T]) -> vk::DeviceSize {
        Self::object_size() * (data.len() as vk::DeviceSize)
    }

    pub fn fill(self: &mut Self, data: &[T]) {
        //! Should panic if the buffer's data manifest could not be made compatible with the data for some reason.
        self.manifest.len = data.len() as u32;
        unsafe {
            let mapped_slice = std::slice::from_raw_parts_mut(self.map, data.len());
            mapped_slice.copy_from_slice(data);
        }
    }
}

impl<T: Copy> Drop for AllocatedDeviceBuffer<T> {
    fn drop(self: &mut Self) {
        unsafe {
            self.dev.unmap_memory(self.mem);
            self.dev.destroy_buffer(self.buffer, None);
            self.dev.free_memory(self.mem, None);
        }
    }
}

pub struct AllocatedDeviceImage {
    pub dev: Arc<VulkanDeviceContext>,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub mem: vk::DeviceMemory,
}

impl AllocatedDeviceImage {
    pub fn new(
        dev: &Arc<VulkanDeviceContext>,
        image_create_info: vk::ImageCreateInfo<'_>,
        mut image_view_create_info: vk::ImageViewCreateInfo<'_>,
        desired_properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let image: vk::Image;
        unsafe {
            image = dev.create_image(&image_create_info, None).unwrap();
        }

        image_view_create_info = image_view_create_info.image(image);
        let image_view: vk::ImageView;
        unsafe {
            image_view = dev
                .create_image_view(&image_view_create_info, None)
                .unwrap();
        }

        let memory_allocate_info = dev
            .reconcile_memory_requirements_with_physical_device_memory_types(
                image,
                desired_properties,
            );
        let dev_mem: vk::DeviceMemory;
        unsafe {
            dev_mem = dev.allocate_memory(&memory_allocate_info, None).unwrap();
            dev.bind_image_memory(image, dev_mem, 0).unwrap();
        }

        return Self {
            dev: Arc::clone(dev),
            image: image,
            image_view: image_view,
            mem: dev_mem,
        };
    }

    pub fn from_preallocated(
        dev: &Arc<VulkanDeviceContext>,
        image: vk::Image,
        mut image_view_create_info: vk::ImageViewCreateInfo<'_>,
    ) -> Self {
        image_view_create_info = image_view_create_info.image(image);
        let image_view: vk::ImageView;
        unsafe {
            image_view = dev
                .create_image_view(&image_view_create_info, None)
                .unwrap();
        }

        return Self {
            dev: Arc::clone(dev),
            image: image,
            image_view: image_view,
            mem: vk::DeviceMemory::default(),
        };
    }

    pub unsafe fn destroy(self: &mut Self) {
        unsafe {
            self.dev.destroy_image_view(self.image_view, None);
            self.dev.destroy_image(self.image, None);
            self.dev.free_memory(self.mem, None);
        }
    }
}

impl Drop for AllocatedDeviceImage {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}

pub trait HasMemReqs {
    fn mem_requirements(self: Self, dev: &ash::Device) -> vk::MemoryRequirements;
}

impl HasMemReqs for vk::MemoryRequirements {
    fn mem_requirements(self: Self, _dev: &ash::Device) -> vk::MemoryRequirements {
        self
    }
}

impl HasMemReqs for vk::Buffer {
    fn mem_requirements(self: Self, dev: &ash::Device) -> vk::MemoryRequirements {
        unsafe { dev.get_buffer_memory_requirements(self) }
    }
}

impl HasMemReqs for vk::Image {
    fn mem_requirements(self: Self, dev: &ash::Device) -> vk::MemoryRequirements {
        unsafe { dev.get_image_memory_requirements(self) }
    }
}

pub struct CmdResources {
    dev: Arc<VulkanDeviceContext>,
    pub pool: vk::CommandPool,
    queue_family_index: u32,
}

impl Deref for CmdResources {
    type Target = vk::CommandPool;
    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

impl CmdResources {
    pub fn new(dev: &Arc<VulkanDeviceContext>) -> Self {
        // TODO Eventually will need to support CONCURRENT sharing mode. Currently only support Exclusive
        // https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain
        let queue_family_index = dev.queue_family_index;
        let pool_create_info: vk::CommandPoolCreateInfo<'_> = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family_index);
        let pool = unsafe { dev.create_command_pool(&pool_create_info, None).unwrap() };
        return CmdResources {
            dev: Arc::clone(dev),
            pool,
            queue_family_index,
        };
    }

    pub fn queue_family_index(self: &Self) -> u32 {
        self.queue_family_index
    }
}

pub struct CmdBufferBatch<const N: usize> {
    res: Arc<CmdResources>,
    buffers: [vk::CommandBuffer; N],
}

impl<const N: usize> CmdBufferBatch<N> {
    pub fn new(res: &Arc<CmdResources>) -> Self {
        let buffer_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(res.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(N as u32);
        let buffers: Vec<vk::CommandBuffer>;
        unsafe {
            buffers = res
                .dev
                .allocate_command_buffers(&buffer_alloc_info)
                .unwrap();
        }
        return CmdBufferBatch {
            res: Arc::clone(res),
            buffers: std::array::from_fn(|i| buffers[i]),
        };
    }

    pub fn queue(self: &Self) -> vk::Queue {
        // TODO eventually this will be determined by sharing (CONCURRENT or EXCLUSIVE) mode rather than by device
        return self.res.dev.queue;
    }

    pub fn as_slice(self: &Self) -> &[vk::CommandBuffer] {
        &self.buffers
    }
}

impl<const N: usize> Drop for CmdBufferBatch<N> {
    fn drop(&mut self) {
        unsafe {
            self.res
                .dev
                .free_command_buffers(self.res.pool, &self.buffers);
        }
    }
}

impl<const N: usize> Index<usize> for CmdBufferBatch<N> {
    type Output = vk::CommandBuffer;
    fn index(&self, index: usize) -> &Self::Output {
        return &self.buffers[index];
    }
}
