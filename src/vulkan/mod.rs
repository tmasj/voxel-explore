use crate::window::{GlfwKernel, WindowLifecycle};
use std::ops::Deref;
use std::sync::Arc;
pub mod device;
use device::*;
pub mod rendering;
mod swapchain;
use rendering::*;

pub struct VulkanLifecycle {
    // TODO (for toplevel clients for use with the Game loop to encapsulate all the Vulkan graphics stuff. Integrates rendering, swapchain updates, etc with the game loop and pushes game state through to GPU buffer mem)
    pub vulkan_kernel: Arc<VulkanKernel>,
    pub device_context: VulkanDeviceContext,
    pub rendering: RenderingFlow,
}

impl VulkanLifecycle {
    pub fn new(windowing: &WindowLifecycle) -> VulkanLifecycle {
        let vk_kern = VulkanKernel::new_from_glfw(&windowing.glfw_kernel);
        let vulkan_kernel = Arc::new(vk_kern);
        let device_context = VulkanDeviceContext::new(Arc::clone(&vulkan_kernel), &windowing);

        let rendering = RenderingFlow::new();

        // TODO fill fields
        VulkanLifecycle {
            vulkan_kernel,
            device_context,
            rendering,
        }
    }
}

impl Drop for VulkanLifecycle {
    fn drop(self: &mut Self) {
        dbg!("Cleanup");
        unsafe {
            vk_ctx
                .device
                .device_wait_idle()
                .expect("Couldn't wait for idle device for cleanup");

            // order should be:
            // self.swapchain
            // self.sync_primitives

            // self.bufsindexed
            // self.graphics_pipeline
            // self.rendering_flow

            // self.device_context
        }
    }
}
