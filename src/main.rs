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

struct ProgramHead;

impl ProgramHead {
    fn new_from_current_instant() -> () {
        let mut windowing = WindowLifecycle::new();
        let mut drawing = VulkanLifecycle::new(&windowing);
        let mut game_global = GameGlobal::new_game_current_instant();
        game_global.event_loop(&mut windowing, &mut drawing.rendering);
        return ();
    }
}

fn main() {
    ProgramHead::new_from_current_instant();
}
