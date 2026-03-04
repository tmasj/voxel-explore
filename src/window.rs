use glfw::fail_on_errors;
use glfw::{Glfw, GlfwReceiver, PWindow, WindowEvent, WindowHint};
use std::ffi::CString;

type WindowEvents = GlfwReceiver<(f64, WindowEvent)>;

pub struct GlfwKernel {
    pub platform: glfw::Platform,
    pub glfw_handle: Glfw,
}

impl GlfwKernel {
    pub fn detect_platform() -> glfw::Platform {
        return if cfg!(target_os = "linux") {
            // Could add env checks here for WSL detection
            glfw::Platform::X11
        } else if cfg!(target_os = "macos") {
            glfw::Platform::MacOS
        } else if cfg!(target_os = "windows") {
            glfw::Platform::Win32
        } else {
            glfw::Platform::Any // fallback
        };
    }

    pub fn vulkan_extension_names(self: &Self) -> Vec<CString> {
        self.glfw_handle
            .get_required_instance_extensions()
            .expect("Failed to get required extensions")
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap())
            .collect::<Vec<CString>>()
    }

    pub fn new_window(self: &mut Self) -> (PWindow, WindowEvents) {
        dbg!(self.glfw_handle.vulkan_supported());
        if !self.glfw_handle.vulkan_supported() {
            panic!("vulkan not supported!!");
        }

        // TODO move out of here to constructor
        self.glfw_handle
            .window_hint(WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
        self.glfw_handle.window_hint(WindowHint::Decorated(true));
        self.glfw_handle.window_hint(WindowHint::Resizable(true));

        let (mut window, events) = self
            .glfw_handle
            .create_window(800, 600, "VOXELEXPLOR", glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window.");

        window.set_key_polling(true);
        window.set_cursor_pos_polling(true);
        window.set_cursor_enter_polling(true);
        window.set_size_polling(true);
        window.set_framebuffer_size_polling(true);
        window.set_close_polling(true);
        window.focus();
        return (window, events);
    }

    pub fn new_from_current_platform() -> Self {
        let platform = Self::detect_platform();
        glfw::init_hint(glfw::InitHint::Platform(platform));
        let glfw_handle = glfw::init(fail_on_errors!()).unwrap();

        return GlfwKernel {
            platform,
            glfw_handle,
        };
    }
}

pub struct WindowLifecycle {
    pub glfw_kernel: GlfwKernel,
    pub window: PWindow,
    pub events: WindowEvents,
}

impl WindowLifecycle {
    pub fn new() -> Self {
        let mut glfw_kernel = GlfwKernel::new_from_current_platform();
        let (window, events) = glfw_kernel.new_window();
        WindowLifecycle {
            glfw_kernel,
            window,
            events,
        }
    }
}
