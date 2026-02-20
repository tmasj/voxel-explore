use crate::geometry_primitives::*;
use crate::vulkan;
use crate::vulkan::rendering::RenderingFlow;
use crate::window::*;
use glfw::{Action, Key}; // for WindowEvents
use std::time;

pub struct GameGlobal {
    last_frame_instant: time::Instant,
}

impl GameGlobal {
    pub fn new_game_current_instant() -> Self {
        Self {
            last_frame_instant: time::Instant::now(),
        }
    }

    fn example_game_geometry(self: &Self) -> IndexedVertexGeometry {}

    pub fn event_loop(self: &mut Self, windowing: &WindowLifecycle, rendering: &RenderingFlow) {
        let mut frameidx = 0;
        const FRAME_DRAW_RETRY_CAP: u8 = 100;
        let mut frame_draw_retries: [u8; MAX_FRAMES_IN_FLIGHT] = [0; MAX_FRAMES_IN_FLIGHT];

        let geom = self.example_game_geometry();
        rendering.load_game_geometry_for_drawing(geom);

        while !windowing.window.should_close() {
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

            windowing.glfw_kernel.glfw_handle.poll_events();

            for (_, event) in glfw::flush_messages(&windowing.events) {
                match event {
                    WindowEvent::Key(Key::W, _, Action::Press, _) => {
                        println!("W!");
                    }
                    WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                        windowing.window.set_should_close(true)
                    }
                    WindowEvent::Size(_width, _height) => {}
                    WindowEvent::FramebufferSize(width, height) => {
                        if width == 0 && height == 0 {
                            'minimized_waiting: loop {
                                windowing.glfw_kernel.glfw_handle.wait_events();
                                for (_, waitingevent) in glfw::flush_messages(&windowing.events) {
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

            self.last_frame_instant = time::Instant::now();
        }
        print!("Exited loop");
    }
}
