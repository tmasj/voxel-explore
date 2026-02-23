use crate::geometry_primitives::*;
use crate::vulkan::device::*;
use crate::vulkan::swapchain;
use ash::vk::{self, PipelineLayout, PushConstantRange};
use std::ffi::CStr;
use std::sync::Arc;

const MAX_FRAMES_IN_FLIGHT: usize = 5;

struct RenderingContextResourceDescriptorSpec {
    dev: Arc<VulkanDeviceContext>,
    layouts: [vk::DescriptorSetLayout; MAX_FRAMES_IN_FLIGHT],
    pool: vk::DescriptorPool,
}

impl RenderingContextResourceDescriptorSpec {
    fn new_one_uniform_buffer(dev: &Arc<VulkanDeviceContext>) -> Self {
        // Each set is identical in layout (binding 0 = UBO), but each one points to a different buffer, one per frame.

        // Layout
        let _samplers: [vk::Sampler; 0] = [];
        let ubo_layout_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX);

        let bindings = [ubo_layout_binding];

        let ubo_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

        let layout;
        unsafe {
            layout = dev
                .create_descriptor_set_layout(&ubo_layout_create_info, None)
                .unwrap();
        }

        // Pool
        let dpsize = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(MAX_FRAMES_IN_FLIGHT as u32);
        let pool_sizes = [dpsize];
        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_FRAMES_IN_FLIGHT as u32)
            .flags(vk::DescriptorPoolCreateFlags::empty());

        let pool: vk::DescriptorPool;
        unsafe {
            pool = dev.create_descriptor_pool(&pool_create_info, None).unwrap();
        }

        Self {
            dev: Arc::clone(dev),
            layouts: [layout; MAX_FRAMES_IN_FLIGHT],
            pool,
        }
    }

    fn allocate_descriptor_set(
        self: &Self,
        allocated: &mut AllocatedDeviceBuffer<UniformBufferObject>,
    ) -> vk::DescriptorSet {
        let set_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool)
            .set_layouts(&self.layouts);
        let descriptor_sets: Vec<vk::DescriptorSet>;
        unsafe {
            descriptor_sets = self.dev.allocate_descriptor_sets(&set_alloc_info).unwrap();
        }
        let [descriptor_set]: [_; 1] = descriptor_sets.try_into().unwrap();

        // Attach descriptor set to buffer
        let buffer_info = vk::DescriptorBufferInfo::default()
            .buffer(allocated.buffer)
            .offset(0)
            .range(AllocatedDeviceBuffer::<UniformBufferObject>::object_size());

        let descriptor_write = vk::WriteDescriptorSet::default()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(std::slice::from_ref(&buffer_info));

        unsafe {
            allocated
                .dev
                .update_descriptor_sets(&[descriptor_write], &[]);
        }

        return descriptor_set;
    }
}

struct RenderingContext {
    dev: Arc<VulkanDeviceContext>,
}

impl RenderingContext {
    fn new(dev: Arc<VulkanDeviceContext>) -> Self {
        Self { dev }
    }

    fn default_push_constant_ranges(self: &Self) -> [PushConstantRange; 0] {
        [] // Just an empty list is adequate for now.
    }

    fn new_pipeline_layout(
        self: &Self,
        set_layouts: &[vk::DescriptorSetLayout],
        push_constant_ranges: &[PushConstantRange],
    ) -> vk::PipelineLayout {
        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        let pipeline_layout;
        unsafe {
            pipeline_layout = self
                .dev
                .create_pipeline_layout(&pipeline_layout_create_info, None)
                .unwrap();
        }

        return pipeline_layout;
    }

    fn allocate_resource_descriptors(self: &Self) -> RenderingContextResourceDescriptorSpec {
        RenderingContextResourceDescriptorSpec::new_one_uniform_buffer(&self.dev)
    }

    fn vertex_shader_module(self: &Self) -> vk::ShaderModule {
        self.dev.shader_module_from_bytes(crate::shader::VERT)
    }

    fn fragment_shader_module(self: &Self) -> vk::ShaderModule {
        self.dev.shader_module_from_bytes(crate::shader::FRAG)
    }

    fn new_pipeline(
        self: &Self,
        extent: vk::Extent2D,
        render_pass: vk::RenderPass,
        pipeline_layout: PipelineLayout,
        vert_shader_mod: vk::ShaderModule,
        frag_shader_mod: vk::ShaderModule,
    ) -> vk::Pipeline {
        // no shader code constants yet
        let specialization_info = vk::SpecializationInfo::default();

        // Vertex Shader setup
        let vert_create_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_shader_mod)
            .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
            .specialization_info(&specialization_info);

        // Frag Shader setup
        let frag_create_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_shader_mod)
            .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
            .specialization_info(&specialization_info);

        let shader_stages = [vert_create_info, frag_create_info];

        // Dynamic States
        let dynamic_states: [vk::DynamicState; 2] =
            [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let viewport = vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(extent.width as f32) // Typically the swapchain extent
            .height(extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        let scissor = vk::Rect2D::default()
            .offset(vk::Offset2D::default())
            .extent(extent);
        let viewports = [viewport];
        let scissors = [scissor];
        let viewport_create_info = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        // Vertex Binding
        let vertex_binding_descriptions = [Vertex::binding_description()];
        let vertex_attribute_descriptions = Vertex::attribute_descriptions();
        let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_attribute_descriptions);
        let pipeline_input_create_info = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        // Rasterization, Sampling, Depth, & Blend
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
        // For now multisampling is off. This has to do with anti-aliasing
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
            graphics_pipeline = self
                .dev
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
                .unwrap();
        }

        if !(graphics_pipeline.len() == 1) {
            panic!("I thought there would be exactly one graphics pipeline...");
        }

        return graphics_pipeline[0];
    }

    fn new_render_pass(
        device: &VulkanDeviceContext,
        color_attach_format: vk::Format,
        depth_stencil_format: vk::Format,
    ) -> vk::RenderPass {
        let color_attachment = vk::AttachmentDescription::default()
            .format(color_attach_format) // likely from the swapchain
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
        let depth_stencil_attachment = vk::AttachmentDescription::default()
            .format(depth_stencil_format)
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

        return render_pass;
    }
}

pub struct RenderingFlow {
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    shader_mod: Vec<vk::ShaderModule>,
    // The Vulkan tutorial (rust version, https://kylemayes.github.io/vulkanalia/model/depth_buffering.html) says:
    // We only need a single depth image, because only one draw operation is running at once.
    // However I think that's not true, at least in my case, so I will err on the side of using 1 DepthBufferSystem per framebuffer/swapchain image
    depth_buffers: Vec<DepthBufferSystem>,
    render_pass: vk::RenderPass,
    // TODO one framebuffer per swapchain image. But it requires both a render pass and a swapchain. Currently render pass requires swapchain, and Swapchain needs to make a SwapchainImage
    // Refactor so the render pass is created in the swapchain to avoid circular dependency
    framebuffers: Vec<vk::Framebuffer>,
    sync_primitives: [SyncPrimitives; MAX_FRAMES_IN_FLIGHT],
    bufs: BufferSystemIndexed,
    dev: Arc<VulkanDeviceContext>,
}

impl RenderingFlow {
    pub fn new(vulkan_device: Arc<VulkanDeviceContext>) -> Self {
        let descriptor_set_layout = UniformBufferObject::descriptor_set_layout(&device);
        let (descriptor_pool, descsets) =
            device_ctxt.create_descriptor_sets_in_new_pool(&device, descriptor_set_layout);

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
                UniformBufferObject::uniform_buffer(
                    &device,
                    &instance,
                    physical_device,
                    descsets[_i],
                )
            })
            .collect();

        // TODO move these to Game
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

        RenderingFlow {
            depth_buffers,
            render_pass,
            pipeline_system,
            framebuffers,
            sync_primitives,
            bufs,
        }
    }

    pub fn load_game_geometry_for_drawing(self: &Self, geometry: IndexedVertexGeometry) {
        load_vertex_data_via_staging_buffer(_vk_ctx, &triangle_vertices_indexed());
        load_index_data_via_staging_buffer(_vk_ctx, &triangle_geom_indices());
    }

    pub fn attempt_next_frame_iter(
        self: &mut Self,
    ) -> impl Iterator<Item = Result<(), vk::Result>> {
        const FRAME_DRAW_RETRY_CAP: u8 = 100;
        let mut frameidx = 0;
        let mut frame_draw_retries: [u8; MAX_FRAMES_IN_FLIGHT] = [0; MAX_FRAMES_IN_FLIGHT];
        return std::iter::from_fn(|| {
            frame_draw_retries[frameidx] += 1;
            if frame_draw_retries[frameidx] > FRAME_DRAW_RETRY_CAP {
                panic!("The frame draw retry cap exceeded");
            }

            let mut drawrslt = draw_frame_by_index(_vk_ctx, frameidx);
            match drawrslt {
                Ok(_) => {
                    frameidx = (frameidx + 1) % MAX_FRAMES_IN_FLIGHT;
                    frame_draw_retries[frameidx] = 0;
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.recreate_swapchain(_vk_ctx);
                    drawrslt = draw_frame_by_index(_vk_ctx, frameidx);
                }
                othererr => panic!("Failed to draw frame: {:?}", othererr),
            };
            return Some(drawrslt);
        });
    }

    #[deprecated]
    pub fn new_vertex_buffer_host_visible(
        self: &Self,
        vertices: &[Vertex],
    ) -> AllocatedDeviceBuffer<Vertex> {
        //! Should use the device-local version
        let create_info = vk::BufferCreateInfo::default()
            .size(AllocatedDeviceBuffer::<Vertex>::data_size(vertices))
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_vertex_buffer_device_local(
        self: &Self,
        vertices: &[Vertex],
    ) -> AllocatedDeviceBuffer<Vertex> {
        let create_info = vk::BufferCreateInfo::default()
            .size(AllocatedDeviceBuffer::<Vertex>::data_size(vertices))
            .usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::DEVICE_LOCAL;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_index_buffer_device_local(
        self: &Self,
        indices: &[GeometryDataIndex],
    ) -> AllocatedDeviceBuffer<GeometryDataIndex> {
        let create_info = vk::BufferCreateInfo::default()
            .size(AllocatedDeviceBuffer::<GeometryDataIndex>::data_size(
                indices,
            ))
            .usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::DEVICE_LOCAL;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_staging_buffer<T: Copy>(self: &Self, data: &[T]) -> AllocatedDeviceBuffer<T> {
        let create_info = vk::BufferCreateInfo::default()
            .size(AllocatedDeviceBuffer::<T>::object_size() * (data.len() as vk::DeviceSize))
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_uniform_buffer(
        device: &Arc<VulkanDeviceContext>,
    ) -> AllocatedDeviceBuffer<UniformBufferObject> {
        let create_info = vk::BufferCreateInfo::default()
            .size(std::mem::size_of::<UniformBufferObject>() as u64)
            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

        return AllocatedDeviceBuffer::new(device, create_info, props);
    }

    pub fn load_data_via_staging_buffer<T: Copy>(
        self: &Self,
        data: &[T],
        dst_buf: &AllocatedDeviceBuffer<T>,
    ) {
        let staging_buf = self.new_staging_buffer::<T>(data);
        let mut staging_map = BufferMemMap::<T>::new(&staging_buf);
        staging_map.fill(data);
        let bufcopy = vk::BufferCopy::default()
            .src_offset(0)
            .dst_offset(0)
            .size(AllocatedDeviceBuffer::<T>::data_size(data));
        self.transfer_buffers_on_device(staging_buf.buffer, dst_buf.buffer, bufcopy);
    }

    pub fn transfer_buffers_on_device(
        self: &Self,
        src_buffer: vk::Buffer,
        dst_buffer: vk::Buffer,
        copy_locations: vk::BufferCopy,
    ) {
        //! TODO
        //!
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
        // let bufcopy = vk::BufferCopy::default()
        //     .src_offset(s)
        //     .dst_offset(0)
        //     .size(size);
        let submit_info = [vk::SubmitInfo::default().command_buffers(&cmd_buffer)];

        unsafe {
            vk_ctx
                .device
                .begin_command_buffer(cmd_buffer[0], &begin_info)
                .unwrap();
            vk_ctx
                .device
                .cmd_copy_buffer(cmd_buffer[0], src_buffer, dst_buffer, &[copy_locations]);
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
}

impl Drop for RenderingFlow {
    fn drop(self: &mut Self) {
        vk_ctx.device.destroy_render_pass(vk_ctx.render_pass, None);
        vk_ctx
            .device
            .destroy_descriptor_pool(vk_ctx.bufs.descriptor_pool, None);
        vk_ctx
            .device
            .destroy_descriptor_set_layout(vk_ctx.bufs.descriptor_set_layout, None);

        for &shader in &vk_ctx.pipeline_system.shader_mod {
            vk_ctx.device.destroy_shader_module(shader, None);
            vk_ctx
                .device
                .destroy_pipeline(vk_ctx.pipeline_system.graphics_pipeline, None);
            vk_ctx
                .device
                .destroy_pipeline_layout(vk_ctx.pipeline_system.pipeline_layout, None);
        }
    }
}

struct SyncPrimitives {
    image_available: vk::Semaphore,
    frame_in_flight: vk::Fence,
}

impl SyncPrimitives {
    fn new() -> Self {
        let sem_create_info = vk::SemaphoreCreateInfo::default();
        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED); // So we can wait on the fence at first frame without blocking
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
}

impl Drop for SyncPrimitives {
    fn drop(&mut self) {
        unsafe {
            for sync_primitive in sync_primitives {
                device.destroy_semaphore(sync_primitive.image_available, None);
                device.destroy_fence(sync_primitive.frame_in_flight, None);
            }
        }
    }
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

impl BufferSystemIndexed {
    fn new() {
        let geom_vert = triangle_vertices_indexed();
        let geom_ind = triangle_geom_indices();
        let (vertex_buffer, devmem_vertex) =
            create_device_local_vertex_buffer(&device, &instance, physical_device, &geom_vert);
        let (index_buffer, devmem_index) =
            create_device_local_index_buffer(&device, &instance, physical_device, &geom_ind);

        let descriptor_set_layout = UniformBufferObject::descriptor_set_layout(&device);
        let (descriptor_pool, descsets) =
            create_descriptor_sets_in_new_pool(&device, descriptor_set_layout);

        let bufs = BufferSystemIndexed {
            devloc_vertex: vertex_buffer,
            vertex_mem: devmem_vertex,
            devloc_index: index_buffer,
            index_mem: devmem_index,
            unibufs: uniform_bufs,
            descriptor_pool: descriptor_pool,
            descriptor_set_layout: descriptor_set_layout,
        };
    }
}

impl Drop for BufferSystemIndexed {
    fn drop(self: &mut Self) {
        vk_ctx
            .device
            .destroy_buffer(vk_ctx.bufs.devloc_vertex, None);
        vk_ctx.device.destroy_buffer(vk_ctx.bufs.devloc_index, None);
        vk_ctx.device.free_memory(vk_ctx.bufs.index_mem, None);
        vk_ctx.device.free_memory(vk_ctx.bufs.vertex_mem, None);
        for i in 0..vk_ctx.bufs.unibufs.len() {
            UniformBufferObject::destroy_uniform_buffer(&vk_ctx.device, &vk_ctx.bufs.unibufs[i]);
        }
    }
}

struct UniformBufSubsys {
    uniform_buffer: vk::Buffer,
    unif_mem: vk::DeviceMemory,
    mapped: *mut UniformBufferObject,
    desc_set: vk::DescriptorSet,
}

impl UniformBufSubsys {
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
    }

    fn destroy_uniform_buffer(device: &ash::Device, uniform_buffer_subsystem: &UniformBufSubsys) {
        unsafe {
            device.destroy_buffer(uniform_buffer_subsystem.uniform_buffer, None);
            device.free_memory(uniform_buffer_subsystem.unif_mem, None);
        }
    }
}

impl Drop for UniformBufSubsys {
    fn drop(self: &mut Self) {
        self.destroy_uniform_buffer();
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct UniformBufferObject {
    model: glam::Mat4,
    view: glam::Mat4, // 64 bytes
    proj: glam::Mat4, // 64 bytes
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

impl Drop for DepthBufferSystem {
    fn drop(self: &mut Self) {
        self.destroy();
    }
}
