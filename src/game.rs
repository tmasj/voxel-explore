use crate::geometry_primitives;
use crate::geometry_primitives::*;
use crate::vulkan::rendering::RenderingFlow;
use crate::window::*;
use glfw::{Action, Key, WindowEvent};
use std::time;

pub struct GameGlobal {
    program_start: time::Instant,
    last_frame_instant: time::Instant,
}

impl GameGlobal {
    pub fn new_game_current_instant() -> Self {
        let now = time::Instant::now();
        Self {
            program_start: now,
            last_frame_instant: now,
        }
    }

    fn example_game_geometry(self: &Self) -> IndexedVertexGeometry {
        IndexedVertexGeometry {
            vertices: geometry_primitives::triangle_vertices_indexed(),
            indices: geometry_primitives::triangle_geom_indices(),
        }
    }

    pub fn event_loop(
        self: &mut Self,
        windowing: &mut WindowLifecycle,
        rendering: &mut RenderingFlow,
    ) {
        self.last_frame_instant = time::Instant::now();
        let geom = self.example_game_geometry();
        rendering.load_game_geometry_for_drawing(geom);
        let mut draw_next_frame_iter = rendering.attempt_next_frame_iter();
        while !windowing.window.should_close() {
            let Some(Ok(_)) = draw_next_frame_iter.next() else {
                panic!("unreachable...")
            };
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
