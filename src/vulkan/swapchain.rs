use crate::vulkan::device::VulkanDeviceContext;
use ash::vk;
use std::sync::Arc;

pub struct SwapchainSurfaceConfig {
    pub dev: Arc<VulkanDeviceContext>,
    pub surface_caps: vk::SurfaceCapabilitiesKHR,
    pub surface_fmt: vk::SurfaceFormatKHR,
    pub min_image_count: u32,
}

impl SwapchainSurfaceConfig {
    fn new(dev: &Arc<VulkanDeviceContext>) -> Self {
        let surface_caps = unsafe {
            dev.surface_loader
                .get_physical_device_surface_capabilities(dev.physical_device, dev.surface)
                .unwrap()
        };
        let surface_fmt = unsafe {
            dev.surface_loader
                .get_physical_device_surface_formats(dev.physical_device, dev.surface)
                .unwrap()[0]
        };

        let min_image_count = Ord::min(
            surface_caps.min_image_count + 1,
            if surface_caps.max_image_count == 0 {
                u32::MAX
            } else {
                surface_caps.max_image_count
            },
        );

        return Self {
            dev: Arc::clone(dev),
            surface_caps,
            surface_fmt,
            min_image_count,
        };
    }

    fn aspect(self: &Self) -> vk::Extent2D {
        // Possibly an unhandled special value https://docs.vulkan.org/refpages/latest/refpages/source/VkSurfaceCapabilitiesKHR.html see also https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain
        self.surface_caps.current_extent
    }

    fn format(self: &Self) -> vk::Format {
        self.surface_fmt.format
    }

    fn swapchain_create_info(self: &Self) -> vk::SwapchainCreateInfoKHR<'_> {
        vk::SwapchainCreateInfoKHR::default()
            .surface(self.dev.surface)
            .min_image_count(self.min_image_count)
            .image_format(self.format())
            .image_color_space(self.surface_fmt.color_space)
            .image_extent(self.aspect())
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE) // See also https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain
            .pre_transform(self.surface_caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO) // Ought to query for supported present modes first
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null()) //Dealloc helper https://vulkan-tutorial.com/Drawing_a_triangle/Swap_chain_recreation
    }

    fn imageless_image_view_create_info_for_swapchain(self: &Self) -> vk::ImageViewCreateInfo<'_> {
        vk::ImageViewCreateInfo::default()
            .image(vk::Image::default())
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(self.format())
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            )
    }
}

pub struct Swapchain {
    pub config: Arc<SwapchainSurfaceConfig>,
    pub swapchain_loader: ash::khr::swapchain::Device,
    pub handle: vk::SwapchainKHR,
}

impl Swapchain {
    pub fn from_device(dev: &Arc<VulkanDeviceContext>) -> Self {
        let config = SwapchainSurfaceConfig::new(dev);
        let arcconfig = Arc::new(config);
        return Self::new(&arcconfig);
    }

    pub fn new(config: &Arc<SwapchainSurfaceConfig>) -> Self {
        let dev = &config.dev;
        let create_info = config.swapchain_create_info();
        let swapchain_loader = ash::khr::swapchain::Device::new(&dev.vulkan_kernel.instance, dev);
        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&create_info, None)
                .unwrap()
        };

        return Self {
            config: Arc::clone(config),
            swapchain_loader,
            handle: swapchain,
        };
    }

    pub fn create_image_views(self: &Self) -> Vec<SwapchainImageView> {
        let images_from_loader: Vec<vk::Image>;
        unsafe {
            images_from_loader = self
                .swapchain_loader
                .get_swapchain_images(self.handle)
                .unwrap();
        }
        return images_from_loader
            .iter()
            .map(|&image| {
                let create_info = self.config.imageless_image_view_create_info_for_swapchain();
                return SwapchainImageView::from_image(&self.config.dev, image, create_info);
            })
            .collect();
    }

    pub fn aspect(self: &Self) -> vk::Extent2D {
        self.config.aspect()
    }

    pub fn format(self: &Self) -> vk::Format {
        self.config.format()
    }

    pub unsafe fn destroy(self: &mut Self) {
        //! Invalidates self
        unsafe {
            self.swapchain_loader.destroy_swapchain(self.handle, None);
            self.handle = Default::default();
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { self.destroy() }
    }
}

pub struct SwapchainImageView {
    dev: Arc<VulkanDeviceContext>,
    pub image_view: vk::ImageView,
}

impl SwapchainImageView {
    pub fn from_image(
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
            image_view: image_view,
        };
    }

    pub unsafe fn destroy(self: &mut Self) {
        unsafe {
            self.dev.destroy_image_view(self.image_view, None);
            self.image_view = Default::default();
        }
    }
}

impl Drop for SwapchainImageView {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}
