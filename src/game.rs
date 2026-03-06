use crate::geometry_primitives::*;
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

    fn basic_voxel(self: &Self) -> Voxel {
        geometry_primitives::Voxel::new(
            Vec3 {
                x: 2.,
                y: 2.,
                z: 0.,
            },
            [0f32, 1.0, 0f32],
        )
    }

    pub fn event_loop(
        self: &mut Self,
        windowing: &mut WindowLifecycle,
        rendering: &mut RenderingFlow,
    ) {
        self.player.rotate_ud(-0.707f32);
        self.player.rotate_lr(1.5);
        self.last_frame_instant = time::Instant::now();
        self.aspect = rendering.aspect();

        let mut vertex_buffer = rendering.new_vertex_buffer_device_local();
        let mut index_buffer = rendering.new_index_buffer_device_local();
        let geom = self.basic_voxel();
        rendering.load_game_geometry_for_drawing(
            IndexedVertexGeometry {
                vertices: geom.vertices(),
                indices: geom.indices(0),
            },
            &mut vertex_buffer,
            &mut index_buffer,
        );
        // TODO this mutable iterator should probably transform into a state machine iterator.
        let mut draw_next_frame_iter = DrawFrameIter::<100>::default();
        while !windowing.window.should_close() {
            match draw_next_frame_iter.attempt_next_frame(
                rendering,
                &vertex_buffer,
                &index_buffer,
                &self.mvp,
            ) {
                Err(_) => {
                    continue;
                }
                Ok(new_aspect) => {
                    self.aspect = new_aspect;
                }
                _ => {
                    panic!("unreachable");
                }
            }
            windowing.glfw_kernel.glfw_handle.poll_events();
            self.tick();

            for (_, event) in glfw::flush_messages(&windowing.events) {
                self.player.handle_window_event(&event);

                match event {
                    WindowEvent::Key(Key::Tab, _, Action::Press, _) => {
                        let next_mode = match windowing.window.get_cursor_mode() {
                            glfw::CursorMode::Normal => glfw::CursorMode::Disabled,
                            _ => glfw::CursorMode::Normal,
                        };
                        windowing.window.set_cursor_mode(next_mode);
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
        let deltat: f32 = self.last_frame_instant.elapsed().as_secs_f32();
        self.player.tick(deltat);

        self.update_mvp(deltat);
    }

    pub fn update_mvp(self: &mut Self, _delta_t: f32) {
        let _elapsedt: f32 = 0.; //self.program_start.elapsed().as_secs_f32();
        let mut unif: UniformBufferObject = UniformBufferObject {
            model: Mat4::from_rotation_z(_elapsedt * 90.0f32.to_radians()),
            view: Mat4::look_at_rh(
                self.player.pos,
                self.player.pos + self.player.front_dir(),
                Vec3::Y,
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
    moving_forward: bool, // TODO moving should be axis * sign, not bool^(# of directions), to control for illegal states
    moving_right: bool,
    moving_left: bool,
    moving_back: bool,
    moving_up: bool,
    moving_down: bool,
    cursor_x: f32, // TODO move into window
    cursor_y: f32,
    dx: f32,
    dy: f32,
    cursor_entered: bool,
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
    fn back_dir(self: &Self) -> Vec3 {
        -(Mat3::from_rotation_y(self.front_yaw) * Vec3::NEG_Z).normalize()
    }

    fn rotate_lr(self: &mut Self, angle_delta: f32) {
        self.front_yaw -= angle_delta;
        self.front_yaw = self.front_yaw.rem_euclid(2.0 * std::f32::consts::PI);
    }

    fn rotate_ud(self: &mut Self, angle_delta: f32) {
        self.front_pitch -= angle_delta;
        self.front_pitch = self.front_pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.05,
            std::f32::consts::FRAC_PI_2 - 0.05,
        );
    }

    fn tick(self: &mut Self, delta_t: f32) {
        const speed: f32 = 0.05;
        let looksens: f32 = 0.005;
        if self.moving_forward {
            self.pos += speed * self.front_dir();
        } else if self.moving_right {
            self.pos += speed * self.right_dir();
        } else if self.moving_left {
            self.pos += speed * self.left_dir();
        } else if self.moving_back {
            self.pos += speed * self.back_dir();
        } else if self.moving_up {
            self.pos += speed * Vec3::Y;
        } else if self.moving_down {
            self.pos += speed * Vec3::NEG_Y;
        }
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
                if !self.cursor_entered {
                    self.dx = dx;
                    self.dy = dy;
                }
                self.cursor_entered = false;
                self.cursor_x = *newx as f32;
                self.cursor_y = *newy as f32;
            }
            WindowEvent::CursorEnter(enter_or_exit) => {
                self.cursor_entered = *enter_or_exit;
                self.dx = 0.;
                self.dy = 0.;
            }
            WindowEvent::Key(Key::W, _, Action::Press, _) => {
                self.moving_forward = true;
            }
            WindowEvent::Key(Key::W, _, Action::Release, _) => {
                self.moving_forward = false;
            }
            WindowEvent::Key(Key::D, _, Action::Press, _) => {
                self.moving_right = true;
            }
            WindowEvent::Key(Key::D, _, Action::Release, _) => {
                self.moving_right = false;
            }
            WindowEvent::Key(Key::S, _, Action::Press, _) => {
                self.moving_back = true;
            }
            WindowEvent::Key(Key::S, _, Action::Release, _) => {
                self.moving_back = false;
            }
            WindowEvent::Key(Key::A, _, Action::Press, _) => {
                self.moving_left = true;
            }
            WindowEvent::Key(Key::A, _, Action::Release, _) => {
                self.moving_left = false;
            }
            WindowEvent::Key(Key::Space, _, Action::Press, _) => {
                self.moving_up = true;
            }
            WindowEvent::Key(Key::Space, _, Action::Release, _) => {
                self.moving_up = false;
            }
            WindowEvent::Key(Key::LeftShift, _, Action::Press, _) => {
                self.moving_down = true;
            }
            WindowEvent::Key(Key::LeftShift, _, Action::Release, _) => {
                self.moving_down = false;
            }
            _ => {}
        }
    }
}
