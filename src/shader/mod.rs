use ash;
use ash::vk;
use std::fs::File;

#[cfg(not(rust_analyzer))]
pub const VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/vert.spv"));
#[cfg(not(rust_analyzer))]
pub const FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/frag.spv"));

#[cfg(rust_analyzer)]
pub const VERT: &[u8] = &[];
#[cfg(rust_analyzer)]
pub const FRAG: &[u8] = &[];

fn parse_spv_data(pathname: impl AsRef<std::path::Path>) -> Vec<u32> {
    let mut flhndl = File::open(pathname).unwrap();
    return ash::util::read_spv(&mut flhndl).unwrap();
}

fn shader_mod_from_spv_path<'a>(
    pathname: impl AsRef<std::path::Path>,
    shader_code: &'a [u32],
) -> vk::ShaderModuleCreateInfo<'a> {
    let create_info = vk::ShaderModuleCreateInfo::default().code(shader_code);
    return create_info;
}
