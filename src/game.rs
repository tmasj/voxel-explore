use crate::geometry_primitives::*;
use crate::vulkan::device::AllocatedDeviceBuffer;
use crate::vulkan::rendering::RenderingFlow;
use crate::window::*;
use crate::{geometry_primitives, vulkan::rendering::DrawFrameIter};
use ash::vk;
use glam::{Mat4, Vec3};
use glfw::{Action, Key, WindowEvent};
use std::time;

pub struct GameGlobal {
    program_start: time::Instant,
    last_frame_instant: time::Instant,
    mvp: UniformBufferObject,
    aspect: vk::Extent2D,
}

impl GameGlobal {
    pub fn new_game_current_instant() -> Self {
        let now = time::Instant::now();
        Self {
            program_start: now,
            last_frame_instant: now,
            mvp: Default::default(),
            aspect: Default::default(),
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
        self.aspect = rendering.aspect();
        let geom = self.example_game_geometry();
        let vertex_buffer = rendering.new_vertex_buffer_device_local();
        let index_buffer = rendering.new_index_buffer_device_local();
        rendering.load_game_geometry_for_drawing(geom, &vertex_buffer, &index_buffer);
        // TODO this mutable iterator should probably transform into a state machine iterator.
        let mut draw_next_frame_iter = DrawFrameIter::<100>::default();
        while !windowing.window.should_close() {
            let new_aspect: vk::Extent2D;
            match draw_next_frame_iter.attempt_next_frame(
                rendering,
                &vertex_buffer,
                &index_buffer,
                &self.mvp,
            ) {
                Err(_) => {
                    continue;
                }
                Ok(newa) => {
                    new_aspect = newa;
                }
                _ => {
                    panic!("unreachable");
                }
            }
            windowing.glfw_kernel.glfw_handle.poll_events();
            self.aspect = new_aspect;
            self.tick();

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

    pub fn tick(self: &mut Self) {
        self.update_mvp();
    }

    pub fn update_mvp(self: &mut Self) {
        let _deltat = self.last_frame_instant.elapsed().as_secs_f32();
        let _elapsedt = self.program_start.elapsed().as_secs_f32();
        let mut unif: UniformBufferObject = UniformBufferObject {
            model: Mat4::from_rotation_z(_elapsedt * 90.0f32.to_radians()),
            view: Mat4::look_at_rh(
                Vec3::new(2.0, 2.0, 2.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            proj: Mat4::perspective_rh(
                45.0f32.to_radians(),
                self.aspect.width as f32 / self.aspect.height as f32,
                0.1,
                10.0,
            ),
        };
        unif.proj.y_axis.y *= -1.0;
        self.mvp = unif;
    }
}
