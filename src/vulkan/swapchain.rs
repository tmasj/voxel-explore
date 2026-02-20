use crate::vulkan::device;

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

impl Swapchain {
    // let swapchain = create_swapchain(
    //         &instance,
    //         &device,
    //         physical_device,
    //         &surface_loader,
    //         surface,
    //         None,
    //     );
    //fn new(vk_dev: &VulkanDeviceContext) {}
    fn new_basic(
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
        let command_resources =
            unsafe { device.create_command_pool(&pool_create_info, None).unwrap() };

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
                    let image_create_info: vk::ImageViewCreateInfo =
                        vk::ImageViewCreateInfo::default()
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
}

impl Drop for Swapchain {
    fn drop(&mut self) {
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
}
