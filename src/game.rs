use crate::geometry_primitives::*;
use crate::vulkan::device::AllocatedDeviceBuffer;
use crate::vulkan::rendering::RenderingFlow;
use crate::window::*;
use crate::{geometry_primitives, vulkan::rendering::DrawFrameIter};
use ash::vk;
use core::f32;
use glam::{Mat3, Mat4, Vec3};
use glfw::{Action, Key, WindowEvent};
use std::time;

pub struct GameGlobal {
    program_start: time::Instant,
    last_frame_instant: time::Instant,
    mvp: UniformBufferObject,
    aspect: vk::Extent2D,
    player: Player,
}

impl GameGlobal {
    pub fn new_game_current_instant() -> Self {
        let now = time::Instant::now();
        Self {
            program_start: now,
            last_frame_instant: now,
            mvp: Default::default(),
            aspect: Default::default(),
            player: Default::default(),
        }
    }

    fn example_game_geometry(self: &Self) -> IndexedVertexGeometry {
        IndexedVertexGeometry {
            vertices: geometry_primitives::triangle_vertices_indexed(),
            indices: geometry_primitives::triangle_geom_indices(),
        }
    }

    fn basic_voxel(self: &Self) -> IndexedVertexGeometry {
        geometry_primitives::Voxel::new(geometry_primitives::Vertex {
            position: [2f32, 2f32, 0f32],
            color: [0f32, 1.0, 0f32],
        })
        .0
    }

    pub fn event_loop(
        self: &mut Self,
        windowing: &mut WindowLifecycle,
        rendering: &mut RenderingFlow,
    ) {
        self.player.rotate_ud(0.707f32);
        self.player
            .rotate_lr(f32::consts::PI * 2. - f32::consts::FRAC_PI_8 * 4.);
        self.last_frame_instant = time::Instant::now();
        self.aspect = rendering.aspect();
        let geom = self.basic_voxel();
        let mut vertex_buffer = rendering.new_vertex_buffer_device_local();
        let mut index_buffer = rendering.new_index_buffer_device_local();
        rendering.load_game_geometry_for_drawing(geom, &mut vertex_buffer, &mut index_buffer);
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
                self.player.handle_window_event(&event);

                match event {
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
        dbg!("Exited loop");
        // Before dropping the buffers, ensure the command buffers are not in use:
        unsafe {
            rendering.dev.queue_wait_idle(rendering.dev.queue).unwrap();
            // TODO buffer lifetimes should be managed by rendering flow
        }
    }

    pub fn tick(self: &mut Self) {
        self.player.tick();

        self.update_mvp();
    }

    pub fn update_mvp(self: &mut Self) {
        let _deltat: f32 = self.last_frame_instant.elapsed().as_secs_f32();
        let _elapsedt: f32 = 0.; //self.program_start.elapsed().as_secs_f32();
        let mut unif: UniformBufferObject = UniformBufferObject {
            model: Mat4::from_rotation_z(_elapsedt * 90.0f32.to_radians()),
            view: Mat4::look_at_rh(
                self.player.pos,
                self.player.pos + self.player.front_dir(),
                Vec3::Y,
                // self.player.pos,
                // ,
                // Vec3::Y,
            ),
            proj: Mat4::perspective_rh(
                45.0f32.to_radians(),
                self.aspect.width as f32 / self.aspect.height as f32,
                1.0,
                5.0,
            ),
        };
        unif.proj.y_axis.y *= -1.0;
        self.mvp = unif;
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Player {
    pos: Vec3,
    front_yaw: f32,
    front_pitch: f32,
    moving_forward: bool,
    moving_right: bool,
    moving_left: bool,
    cursor_x: f32, // TODO move into window
    cursor_y: f32,
    dx: f32,
    dy: f32,
    entered: bool,
}

impl Player {
    fn front_dir(self: &Self) -> Vec3 {
        (Mat3::from_rotation_y(self.front_yaw)
            * Mat3::from_rotation_x(self.front_pitch)
            * Vec3::NEG_Z)
            .normalize()
    }
    fn left_dir(self: &Self) -> Vec3 {
        (Mat3::from_rotation_y(self.front_yaw + std::f32::consts::FRAC_PI_2) * Vec3::NEG_Z)
            .normalize()
    }
    fn right_dir(self: &Self) -> Vec3 {
        (Mat3::from_rotation_y(self.front_yaw - std::f32::consts::FRAC_PI_2) * Vec3::NEG_Z)
            .normalize()
    }

    fn rotate_lr(self: &mut Self, angle_delta: f32) {
        self.front_yaw -= angle_delta;
        self.front_yaw = self.front_yaw.rem_euclid(2.0 * std::f32::consts::PI);
    }

    fn rotate_ud(self: &mut Self, angle_delta: f32) {
        self.front_pitch -= angle_delta;
        self.front_pitch = self.front_pitch.rem_euclid(2.0 * std::f32::consts::PI);
    }

    fn tick(self: &mut Self) {
        const speed: f32 = 0.05;
        let looksens: f32 = 0.005;
        if self.moving_forward {
            self.pos += speed * self.front_dir();
        } else if self.moving_right {
            self.pos += speed * self.right_dir();
        } else if self.moving_left {
            self.pos += speed * self.left_dir();
        }
        // dbg!((self.dx, self.dy));
        self.rotate_lr((self.dx as f32) * looksens);
        self.rotate_ud((self.dy as f32) * looksens);
        self.dx = 0.;
        self.dy = 0.;
    }

    fn handle_window_event(self: &mut Self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorPos(newx, newy) => {
                let (dx, dy) = (
                    (*newx as f32) - self.cursor_x,
                    (*newy as f32) - self.cursor_y,
                );
                if !self.entered {
                    self.dx = dx;
                    self.dy = dy;
                }
                self.entered = false;
                self.cursor_x = *newx as f32;
                self.cursor_y = *newy as f32;
            }
            WindowEvent::CursorEnter(enter_or_exit) => {
                self.entered = *enter_or_exit;
                self.dx = 0.;
                self.dy = 0.;
            }
            WindowEvent::Key(Key::W, _, Action::Press, _) => {
                dbg!(event);
                self.moving_forward = true;
            }
            WindowEvent::Key(Key::W, _, Action::Release, _) => {
                dbg!(event);
                self.moving_forward = false;
            }
            WindowEvent::Key(Key::D, _, Action::Press, _) => {
                self.moving_right = true;
            }
            WindowEvent::Key(Key::D, _, Action::Release, _) => {
                self.moving_right = false;
            }
            WindowEvent::Key(Key::A, _, Action::Press, _) => {
                self.moving_left = true;
            }
            WindowEvent::Key(Key::A, _, Action::Release, _) => {
                self.moving_left = false;
            }
            _ => {}
        }
    }
}
