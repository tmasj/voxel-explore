use crate::geometry_primitives::*;
use crate::vulkan::device::*;
use crate::vulkan::swapchain::Swapchain;
use ash::vk;
use core::slice;
use std::ffi::CStr;
use std::sync::Arc;

const MAX_FRAMES_IN_FLIGHT: usize = 5;
const BUFFER_DATA_BYTE_COUNT_UPPER_BOUND: vk::DeviceSize = 65536; // 64 KiB. Window's minimum allocation granularity

struct RenderingContextResourceDescriptorSpec {
    dev: Arc<VulkanDeviceContext>,
    pub layouts: [vk::DescriptorSetLayout; MAX_FRAMES_IN_FLIGHT],
    pub pool: vk::DescriptorPool,
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

    fn allocate_and_attach_descriptor_set(
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

    pub unsafe fn destroy(self: &mut Self) {
        for &layout in &self.layouts {
            self.dev.destroy_descriptor_set_layout(layout, None);
        }
        self.dev.destroy_descriptor_pool(self.pool, None);
    }
}

impl Drop for RenderingContextResourceDescriptorSpec {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}

struct RenderingContext {
    dev: Arc<VulkanDeviceContext>,
}

impl RenderingContext {
    fn new(dev: &Arc<VulkanDeviceContext>) -> Self {
        Self {
            dev: Arc::clone(dev),
        }
    }

    fn default_push_constant_ranges(self: &Self) -> [vk::PushConstantRange; 0] {
        [] // Just an empty list is adequate for now.
    }

    fn new_pipeline_layout(
        self: &Self,
        set_layouts: &[vk::DescriptorSetLayout],
        push_constant_ranges: &[vk::PushConstantRange],
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

    fn resource_descriptor_spec(self: &Self) -> RenderingContextResourceDescriptorSpec {
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
        pipeline_layout: vk::PipelineLayout,
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
        self: &Self,
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
            render_pass = self
                .dev
                .create_render_pass(&render_pass_create_info, None)
                .unwrap();
        }

        return render_pass;
    }

    pub fn depth_buffer_format(self: &Self) -> vk::Format {
        vk::Format::D32_SFLOAT //TODO query hardware for supported format
    }

    pub fn clear_values(self: &Self) -> [vk::ClearValue; 2] {
        // TODO query hardware for supported ClearValues
        [
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
        ]
    }
}

pub struct RenderPassAttachments {
    dev: Arc<VulkanDeviceContext>,
    // These are all per-swapchain image
    pub framebuffers: Vec<vk::Framebuffer>,
    // The Vulkan tutorial (rust version, https://kylemayes.github.io/vulkanalia/model/depth_buffering.html) says:
    // We only need a single depth image, because only one draw operation is running at once.
    // However I think that's not true, at least in my case, so I will err on the side of using 1 DepthBufferSystem per framebuffer/swapchain image
    pub depth_buffers: Vec<AllocatedDeviceImage>,
    pub swapchain_images: Vec<AllocatedDeviceImage>,
}

impl RenderPassAttachments {
    pub fn new(
        dev: &Arc<VulkanDeviceContext>,
        render_pass: vk::RenderPass,
        swapchain: &Swapchain,
    ) -> Self {
        let format = swapchain.format();
        let aspect = swapchain.aspect();
        let swapchain_images = swapchain.images();
        let depth_buffers: Vec<AllocatedDeviceImage> = swapchain_images
            .iter()
            .map(|_| Self::new_depth_buffer_no_stencil(dev, aspect, format))
            .collect();
        let framebuffers: Vec<vk::Framebuffer> = swapchain_images
            .iter()
            .zip(depth_buffers.iter())
            .map(|(swim, depbuf)| {
                let attachments = [swim.image_view, depbuf.image_view];
                return Self::new_framebuffer(dev, render_pass, &attachments, aspect);
            })
            .collect();

        return Self {
            dev: Arc::clone(dev),
            framebuffers,
            depth_buffers,
            swapchain_images,
        };
    }

    pub fn new_framebuffer(
        dev: &Arc<VulkanDeviceContext>,
        render_pass: vk::RenderPass,
        attachments: &[vk::ImageView],
        aspect: vk::Extent2D,
    ) -> vk::Framebuffer {
        // TODO combine responsibility for the layout of 'attachments' with the binding refs speced in RenderingContext. Rendering context should likely defer to Self for binding ref indices.

        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass) // The lifetime of the underlyng render pass referenced by the RenderPass numeric handle should be owned by VulkanContext. so there is an implied &'a' here for the lifetime of the latent RenderPass in the driver.
            .attachments(&attachments)
            .width(aspect.width)
            .height(aspect.height)
            .layers(1);

        return unsafe { dev.create_framebuffer(&framebuffer_info, None).unwrap() };
    }

    pub fn new_depth_buffer_no_stencil(
        dev: &Arc<VulkanDeviceContext>,
        aspect: vk::Extent2D,
        format: vk::Format,
    ) -> AllocatedDeviceImage {
        let queue_family_indices = [];
        let image_create_info = vk::ImageCreateInfo::default()
            .flags(vk::ImageCreateFlags::empty())
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: aspect.width,
                height: aspect.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queue_family_indices)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image_view_create_info: vk::ImageViewCreateInfo = vk::ImageViewCreateInfo::default()
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );

        let desired_properties = vk::MemoryPropertyFlags::DEVICE_LOCAL;

        return AllocatedDeviceImage::new(
            dev,
            image_create_info,
            image_view_create_info,
            desired_properties,
        );
    }

    pub unsafe fn destroy(self: &mut Self) {
        //! Must wait for swapchain images to be no longer in use. It's expected that the RenderFlow call queue_wait_idle() before this.
        //! This invalidates the images owned by the swapchain. TODO Consider creating a separate SwapchainImageView struct so this is no longer the case.
        unsafe {
            for &framebuffer in &self.framebuffers {
                self.dev.destroy_framebuffer(framebuffer, None);
            }
            for depth_image in &mut self.depth_buffers {
                depth_image.destroy();
            }
            for swapchain_image in &mut self.swapchain_images {
                swapchain_image.destroy();
            }
        }
    }
}

impl Drop for RenderPassAttachments {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}

pub struct RenderingFlow {
    // Uniform Buffering
    uniform_buffer: AllocatedDeviceBuffer<UniformBufferObject>,
    mvp_descriptor_spec: RenderingContextResourceDescriptorSpec,
    mvp_descriptor: vk::DescriptorSet,

    // Rendering Context
    pipeline: vk::Pipeline,
    vert_shader: vk::ShaderModule,
    frag_shader: vk::ShaderModule,
    pipeline_layout: vk::PipelineLayout,

    render_pass: vk::RenderPass,
    render_pass_attachments: RenderPassAttachments,

    // The vulkan context holds a present queue. That queue likely the same as the one these are assigned to but not necessarily. TODO support CONCURRENT mode
    cmd_resources: Arc<CmdResources>,
    cmd_buffers: Vec<CmdBufferBatch<1>>, // One per Swapchain image

    signal_frame_begin: [vk::Fence; MAX_FRAMES_IN_FLIGHT],
    signal_image_avail: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    signal_render_finished: Vec<vk::Semaphore>,

    context: RenderingContext,
    swapchain: Swapchain,
    dev: Arc<VulkanDeviceContext>,
}

impl RenderingFlow {
    pub fn new(dev: &Arc<VulkanDeviceContext>) -> Self {
        let swapchain = Swapchain::from_device(dev);
        let mut uniform_buffer = Self::new_uniform_buffer(dev);
        let context = RenderingContext::new(dev);
        let mvp_descriptor_spec = context.resource_descriptor_spec();
        let mvp_descriptor =
            mvp_descriptor_spec.allocate_and_attach_descriptor_set(&mut uniform_buffer);

        let render_pass: vk::RenderPass =
            context.new_render_pass(swapchain.format(), context.depth_buffer_format());
        let render_pass_attachments = RenderPassAttachments::new(&dev, render_pass, &swapchain);

        let (vert_shader, frag_shader) = (
            context.vertex_shader_module(),
            context.fragment_shader_module(),
        );
        let push_constant_ranges = [];
        let pipeline_layout =
            context.new_pipeline_layout(&mvp_descriptor_spec.layouts, &push_constant_ranges);
        let pipeline = context.new_pipeline(
            swapchain.aspect(),
            render_pass,
            pipeline_layout,
            vert_shader,
            frag_shader,
        );

        let signal_frame_begin: [vk::Fence; MAX_FRAMES_IN_FLIGHT] =
            std::array::from_fn(|_| Self::new_signalled_fence_frame_in_flight(dev));
        let signal_image_avail: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT] =
            std::array::from_fn(|_| Self::new_semaphore_image_available(dev));
        let signal_render_finished: Vec<vk::Semaphore> =
            (0..render_pass_attachments.swapchain_images.len())
                .map(|_| Self::new_semaphore_render_finished(dev))
                .collect();

        let cmd_resources = Arc::new(CmdResources::new(dev));
        let cmd_buffers: Vec<CmdBufferBatch<1>> =
            (0..render_pass_attachments.swapchain_images.len())
                .map(|_| CmdBufferBatch::<1>::new(&cmd_resources))
                .collect();

        RenderingFlow {
            context,
            swapchain,
            uniform_buffer,
            mvp_descriptor_spec,
            mvp_descriptor,
            pipeline,
            vert_shader,
            frag_shader,
            pipeline_layout,
            render_pass,
            render_pass_attachments,
            cmd_resources,
            cmd_buffers,
            signal_frame_begin,
            signal_image_avail,
            signal_render_finished,
            dev: Arc::clone(dev),
        }
    }

    pub fn aspect(self: &Self) -> vk::Extent2D {
        self.swapchain.aspect()
    }

    pub fn load_game_geometry_for_drawing(
        self: &Self,
        geometry: IndexedVertexGeometry,
        vertex_buffer: &AllocatedDeviceBuffer<Vertex>,
        index_buffer: &AllocatedDeviceBuffer<GeometryDataIndex>,
    ) {
        self.load_data_via_staging_buffer::<Vertex>(&geometry.vertices, &vertex_buffer);
        self.load_data_via_staging_buffer::<GeometryDataIndex>(&geometry.indices, &index_buffer);
    }

    #[deprecated]
    pub fn new_vertex_buffer_host_visible(self: &Self) -> AllocatedDeviceBuffer<Vertex> {
        //! Should use the device-local version
        let create_info = vk::BufferCreateInfo::default()
            .size(BUFFER_DATA_BYTE_COUNT_UPPER_BOUND)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_vertex_buffer_device_local(self: &Self) -> AllocatedDeviceBuffer<Vertex> {
        let create_info = vk::BufferCreateInfo::default()
            .size(BUFFER_DATA_BYTE_COUNT_UPPER_BOUND)
            .usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::DEVICE_LOCAL;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_index_buffer_device_local(self: &Self) -> AllocatedDeviceBuffer<GeometryDataIndex> {
        let create_info = vk::BufferCreateInfo::default()
            .size(BUFFER_DATA_BYTE_COUNT_UPPER_BOUND)
            .usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .flags(vk::BufferCreateFlags::empty());
        let props = vk::MemoryPropertyFlags::DEVICE_LOCAL;

        return AllocatedDeviceBuffer::new(&self.dev, create_info, props);
    }

    pub fn new_staging_buffer<T: Copy>(self: &Self) -> AllocatedDeviceBuffer<T> {
        let create_info = vk::BufferCreateInfo::default()
            .size(BUFFER_DATA_BYTE_COUNT_UPPER_BOUND)
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
        let mut staging_buf = self.new_staging_buffer::<T>();
        staging_buf.fill(data);
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
        let cmd_buffer = CmdBufferBatch::<1>::new(&self.cmd_resources);
        let inheritance_info: vk::CommandBufferInheritanceInfo<'_> =
            vk::CommandBufferInheritanceInfo::default();
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .inheritance_info(&inheritance_info);
        let submit_info = [vk::SubmitInfo::default().command_buffers(cmd_buffer.as_slice())];

        unsafe {
            self.dev
                .begin_command_buffer(cmd_buffer[0], &begin_info)
                .unwrap();
            self.dev
                .cmd_copy_buffer(cmd_buffer[0], src_buffer, dst_buffer, &[copy_locations]);
            self.dev.end_command_buffer(cmd_buffer[0]).unwrap();
            // TODO likely I should wait on all fences here...
            // TODO can use a separate queue for this transfer, if additional concurrency is desired
            self.dev
                .queue_submit(cmd_buffer.queue(), &submit_info, vk::Fence::null())
                .unwrap();
            self.dev.queue_wait_idle(cmd_buffer.queue()).unwrap();
        }
    }

    fn record_command_buffer(
        self: &mut Self,
        image_index: u32,
        vertex_buffer: &AllocatedDeviceBuffer<Vertex>,
        index_buffer: &AllocatedDeviceBuffer<GeometryDataIndex>,
    ) {
        let inheritance_info: vk::CommandBufferInheritanceInfo<'_> =
            vk::CommandBufferInheritanceInfo::default();
        let cmd_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .inheritance_info(&inheritance_info);
        let cmd_buffer_target = self.cmd_buffers[image_index as usize][0];
        unsafe {
            self.dev
                .reset_command_buffer(cmd_buffer_target, vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.dev
                .begin_command_buffer(cmd_buffer_target, &cmd_buffer_begin_info)
                .unwrap();
        }

        unsafe {
            let clear_values = self.context.clear_values();
            let aspect = self.aspect();
            let render_pass_begin_info = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass) //implicitly borrowed
                .framebuffer(self.render_pass_attachments.framebuffers[image_index as usize])
                .render_area(
                    vk::Rect2D::default()
                        .offset(vk::Offset2D { x: 0, y: 0 })
                        .extent(aspect),
                )
                .clear_values(&clear_values);
            self.dev.cmd_begin_render_pass(
                cmd_buffer_target,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            self.dev.cmd_bind_pipeline(
                cmd_buffer_target,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            // We already set these up in create_graphics_pipeline, but we reinclude them here since we set the pipeline to set these dynamically.
            let viewport = vk::Viewport::default()
                .x(0.0)
                .y(0.0)
                .width(aspect.width as f32)
                .height(aspect.height as f32)
                .min_depth(0.0)
                .max_depth(1.0);
            let scissor = vk::Rect2D::default()
                .offset(vk::Offset2D::default())
                .extent(aspect);
            let viewports = [viewport];
            let scissors = [scissor];
            self.dev.cmd_set_viewport(cmd_buffer_target, 0, &viewports);
            self.dev.cmd_set_scissor(cmd_buffer_target, 0, &scissors);

            // Draw this
            let vertex_buffers: [vk::Buffer; 1] = [vertex_buffer.buffer];
            let offsets = [0];
            self.dev
                .cmd_bind_vertex_buffers(cmd_buffer_target, 0, &vertex_buffers, &offsets);
            self.dev.cmd_bind_index_buffer(
                cmd_buffer_target,
                index_buffer.buffer,
                0,
                GeometryDataIndexVkType,
            );

            let descriptor_sets = [self.mvp_descriptor];
            let dynamic_offsets = [];
            self.dev.cmd_bind_descriptor_sets(
                cmd_buffer_target,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &descriptor_sets,
                &dynamic_offsets,
            );
            self.dev.cmd_draw_indexed(
                cmd_buffer_target,
                index_buffer.manifest.len(),
                1,
                index_buffer.manifest.offset_unsigned(),
                vertex_buffer.manifest.offset(),
                0,
            );

            self.dev.cmd_end_render_pass(cmd_buffer_target);
            self.dev
                .end_command_buffer(cmd_buffer_target)
                .expect("An error occured while drawing");
        }
    }

    fn draw_frame_by_index(
        self: &mut Self,
        frameidx: usize,
        vertex_buffer: &AllocatedDeviceBuffer<Vertex>,
        index_buffer: &AllocatedDeviceBuffer<GeometryDataIndex>,
        mvp: &UniformBufferObject,
    ) -> Result<(), vk::Result> {
        unsafe {
            self.dev
                .wait_for_fences(&[self.signal_frame_begin[frameidx]], true, u64::MAX)
                .expect("Failed to wait for the fence");
            // 1. Get next swapchain image
            self.dev
                .reset_fences(&[self.signal_frame_begin[frameidx]])
                .expect("Failed to reset fence");

            let (image_index, _) = self.swapchain.swapchain_loader.acquire_next_image(
                self.swapchain.handle,
                u64::MAX, // No timeout
                self.signal_image_avail[frameidx],
                vk::Fence::null(), // No fence
            )?;

            // 2. Record and submit your draw commands
            self.record_command_buffer(image_index, vertex_buffer, index_buffer);
            self.update_uniform_buffer(mvp);

            // 3. Submit to GPU
            let command_buffers = [self.cmd_buffers[image_index as usize][0]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let queue_submit_wait_semaphores = [self.signal_image_avail[frameidx]];
            // See https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html
            // The submit finishing does not imply image presentation finished, so Vulkan cannot guarantee the semaphore is not still in use unless either:
            // 1. an extension is used to give image_present a completion sync (lame), or
            // 2. you ensure you don't reuse a semaphore until the image is next available (which does ensure the semaphore is free)
            // I choose 2, so queue_submit signal semaphores must be indexed by swapch image
            let queue_submit_signal_semaphores = [self.signal_render_finished[frameidx]];
            let submit_info: vk::SubmitInfo<'_> = vk::SubmitInfo::default()
                .command_buffers(&command_buffers)
                .wait_dst_stage_mask(&wait_stages)
                .wait_semaphores(&queue_submit_wait_semaphores)
                .signal_semaphores(&queue_submit_signal_semaphores);
            let submits = [submit_info];

            self.dev
                .queue_submit(self.dev.queue, &submits, self.signal_frame_begin[frameidx])
                .expect("Failed to submit draw command buffer");

            // 4. Present the image
            let swapchains = [self.swapchain.handle];
            let indices = [image_index];
            let present_info = vk::PresentInfoKHR::default()
                .swapchains(&swapchains)
                .image_indices(&indices)
                .wait_semaphores(&queue_submit_signal_semaphores);

            self.swapchain
                .swapchain_loader
                .queue_present(self.dev.queue, &present_info)?;
        }

        return Ok(());
    }

    fn update_uniform_buffer(self: &mut Self, mvp: &UniformBufferObject) {
        self.uniform_buffer.fill(slice::from_ref(mvp));
    }

    fn recreate_swapchain(self: &mut Self) {
        // clear the graphics pipeline
        // Technically, a wait_for_fences could deadlock here if a submit failed.
        // The Vulkan spec demands that a failing queue_submit() cannot alter resource states.
        // device_wait_idle here would simply stall work until gpu compute finishes, which is unbounded
        // Not doing anything with the fences risks waiting on them unsignalled.
        // So since I can't just signal them from host side TODO I need to recreate the fences (safe after the queue completed whether or not signalled).
        unsafe {
            self.dev.queue_wait_idle(self.dev.queue).unwrap();
        }
        // TODO Recreate Fences...
        // TODO Don't touch the semaphore that is used for presentation...
        unsafe {
            self.render_pass_attachments.destroy();
            self.swapchain.destroy();
        }
        self.swapchain = Swapchain::from_device(&self.dev);
        self.render_pass_attachments =
            RenderPassAttachments::new(&self.dev, self.render_pass, &self.swapchain)
    }

    fn new_semaphore_image_available(dev: &VulkanDeviceContext) -> vk::Semaphore {
        let sem_create_info = vk::SemaphoreCreateInfo::default();
        unsafe {
            return dev.create_semaphore(&sem_create_info, None).unwrap();
        }
    }
    fn new_semaphore_render_finished(dev: &VulkanDeviceContext) -> vk::Semaphore {
        let sem_create_info = vk::SemaphoreCreateInfo::default();
        unsafe {
            return dev.create_semaphore(&sem_create_info, None).unwrap();
        }
    }
    fn new_signalled_fence_frame_in_flight(dev: &VulkanDeviceContext) -> vk::Fence {
        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED); // So we can wait on the fence at first frame without blocking
        // https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html
        unsafe {
            return dev.create_fence(&fence_create_info, None).unwrap();
        }
    }
}

impl Drop for RenderingFlow {
    fn drop(self: &mut Self) {
        unsafe {
            self.dev.destroy_render_pass(self.render_pass, None);
            self.dev.destroy_pipeline(self.pipeline, None);
            for &module in &[self.vert_shader, self.frag_shader] {
                self.dev.destroy_shader_module(module, None);
            }
            self.dev.destroy_pipeline_layout(self.pipeline_layout, None);
            for &fence in &self.signal_frame_begin {
                self.dev.destroy_fence(fence, None);
            }
            for &sem in &self.signal_image_avail {
                self.dev.destroy_semaphore(sem, None);
            }
            for &sem in &self.signal_render_finished {
                self.dev.destroy_semaphore(sem, None);
            }
        }
    }
}

#[derive(Default)]
pub struct DrawFrameIter<const FRAME_DRAW_RETRY_CAP: u8> {
    frameidx: usize,
    frame_draw_retries: [u8; MAX_FRAMES_IN_FLIGHT],
}
impl<const FRAME_DRAW_RETRY_CAP: u8> DrawFrameIter<FRAME_DRAW_RETRY_CAP> {
    pub fn attempt_next_frame(
        self: &mut Self,
        render_flow: &mut RenderingFlow,
        vertex_buffer: &AllocatedDeviceBuffer<Vertex>,
        index_buffer: &AllocatedDeviceBuffer<GeometryDataIndex>,
        mvp: &UniformBufferObject,
    ) -> Result<vk::Extent2D, vk::Result> {
        self.frame_draw_retries[self.frameidx] += 1;
        if self.frame_draw_retries[self.frameidx] > FRAME_DRAW_RETRY_CAP {
            panic!("The frame draw retry cap exceeded");
        }

        let drawrslt =
            render_flow.draw_frame_by_index(self.frameidx, vertex_buffer, index_buffer, mvp);
        match drawrslt {
            Ok(_) => {
                self.frameidx = (self.frameidx + 1) % MAX_FRAMES_IN_FLIGHT;
                self.frame_draw_retries[self.frameidx] = 0;
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                render_flow.recreate_swapchain();
            }
            othererr => panic!("Failed to draw frame: {:?}", othererr),
        };
        return drawrslt.map(|_| render_flow.aspect());
    }
}
