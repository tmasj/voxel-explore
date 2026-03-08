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

#[cfg(not(rust_analyzer))]
pub const ATMOSV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/atmos_v.spv"));
#[cfg(not(rust_analyzer))]
pub const ATMOSF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/atmos_f.spv"));

#[cfg(rust_analyzer)]
pub const ATMOSV: &[u8] = &[];
#[cfg(rust_analyzer)]
pub const ATMOSF: &[u8] = &[];

fn parse_spv_data(pathname: impl AsRef<std::path::Path>) -> Vec<u32> {
    let mut flhndl = File::open(pathname).unwrap();
    return ash::util::read_spv(&mut flhndl).unwrap();
}
