// #![allow(unused_imports)]
// #![allow(dead_code)]
use ash::vk;
use glam;
use glam::{Mat4, Vec3};

use std::time;
mod game;
use game::*;
mod geometry_primitives;
mod shader;
mod window;
use window::*;
mod vulkan;
use vulkan::*;

struct ProgramHead {
    //program_start: time::Instant,
}

impl ProgramHead {
    fn new_from_current_instant() -> () {
        let program_start = time::Instant::now();
        let windowing = WindowLifecycle::new();
        let drawing = VulkanLifecycle::new(&windowing);
        let mut game_global = GameGlobal::new_game_current_instant();
        game_global.event_loop(&windowing, &drawing.rendering);
        return ();
    }
}

fn main() {
    let head = ProgramHead::new_from_current_instant();
}
