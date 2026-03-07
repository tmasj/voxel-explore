use ash::vk;

pub struct IndexedMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<VertexIdx>,
}

pub type VertexIdx = u16; // Can be u16 or u32 
pub const VERTEXIDX_VK_TYPE: vk::IndexType = vk::IndexType::UINT16;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3], // From now on, each vertex applies to only one face
} // Could add texture coords, ambient occlusion, etc.

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl Vertex {
    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
    }
    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
        let first = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, position) as u32);
        let second = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, color) as u32);
        let third = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(2)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, normal) as u32);

        return [first, second, third];
    }
}

// Deprecated
// fn get_triangle_geometry() -> Vec<Vertex> {
//     vec![
//         Vertex {
//             position: [0.0, -0.5, 0.0],
//             color: [1.0, 0.0, 0.2],
//         },
//         Vertex {
//             position: [0.5, 0.5, 0.0],
//             color: [0.1, 0.5, 1.0],
//         },
//         Vertex {
//             position: [-0.5, 0.5, 0.0],
//             color: [0.7, 0.5, 0.0],
//         },
//     ]
// }

// fn get_triangle_geometry2() -> Vec<Vertex> {
//     vec![
//         Vertex {
//             position: [0.5, -0.5, 0.0],
//             color: [1.0, 0.0, 0.2],
//         },
//         Vertex {
//             position: [0.0, 0.5, 0.0],
//             color: [0.1, 0.5, 1.0],
//         },
//         Vertex {
//             position: [0.5, 0.5, 0.0],
//             color: [0.7, 0.5, 0.0],
//         },
//     ]
// }

// pub fn triangle_vertices_indexed() -> Vec<Vertex> {
//     vec![
//         Vertex {
//             position: [-0.5, -0.5, 0.0],
//             color: [1.0, 0.0, 0.0],
//         },
//         Vertex {
//             position: [0.5, -0.5, 0.0],
//             color: [0.0, 1.0, 0.0],
//         },
//         Vertex {
//             position: [0.5, 0.5, 0.0],
//             color: [0.0, 0.0, 1.0],
//         },
//         Vertex {
//             position: [-0.5, 0.5, 0.0],
//             color: [1.0, 1.0, 1.0],
//         },
//         Vertex {
//             position: [-0.5, -0.5, -0.5],
//             color: [1.0, 0.0, 0.0],
//         },
//         Vertex {
//             position: [0.5, -0.5, -0.5],
//             color: [0.0, 1.0, 0.0],
//         },
//         Vertex {
//             position: [0.5, 0.5, -0.5],
//             color: [0.0, 0.0, 1.0],
//         },
//         Vertex {
//             position: [-0.5, 0.5, -0.5],
//             color: [1.0, 1.0, 1.0],
//         },
//     ]
// }

// pub fn triangle_geom_indices() -> Vec<GeometryDataIndex> {
//     vec![0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4]
// }

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct UniformBufferObject {
    pub model: glam::Mat4,
    pub view: glam::Mat4, // 64 bytes
    pub proj: glam::Mat4, // 64 bytes
}

use glam::Vec3;

/// A CCW-wound triangle (front face = CCW with back-face culling).
/// `former` and `latter` are edge vectors from `position`;
/// `latter` is CCW from `former` when viewed from the front.
/// Normal = former × latter (points toward viewer on the front face).
pub struct TriangleCCW {
    pub position: Vec3, // first vertex
    pub former: Vec3,   // edge to second vertex
    pub latter: Vec3,   // edge to third vertex (CCW from former)
}

impl TriangleCCW {
    pub fn normal(&self) -> Vec3 {
        self.former.cross(self.latter).normalize()
    }

    pub fn vertices(&self, color: [f32; 3]) -> [Vertex; 3] {
        let n = self.normal().to_array();
        [(0., 0.), (1., 0.), (0., 1.)].map(|(f, l)| Vertex {
            position: (self.position + self.former * f + self.latter * l).to_array(),
            color,
            normal: n,
        })
    }

    /// Flat indices for a single triangle: [0, 1, 2] offset by `base`.
    pub fn indices(base: VertexIdx) -> [VertexIdx; 3] {
        [base, base + 1, base + 2]
    }
}

/// A CCW-wound quad (two triangles). `former` and `latter` are edge vectors;
/// `latter` is CCW from `former` when viewed from the front.
/// Normal = former × latter.
///
/// Corners:
///   0: position
///   1: position + former
///   2: position + former + latter
///   3: position + latter
///
/// Triangles (CCW): [0,1,2] and [2,3,0]
pub struct QuadCCW {
    pub position: Vec3,
    pub former: Vec3,
    pub latter: Vec3,
}

impl QuadCCW {
    pub fn normal(&self) -> Vec3 {
        // Claude says:
        // The right-hand rule and CCW winding are deeply linked — if you curl your right hand fingers in the CCW direction of the vertices as seen from the front, your thumb points toward you, which is the outward normal direction. So former.cross(latter) naturally gives you the normal pointing toward the viewer on the front face. CCW is just the "natural" choice that keeps the geometry algebra and the lighting algebra in sync with no sign correction needed.
        self.former.cross(self.latter).normalize()
    }

    pub fn vertices(&self, color: [f32; 3]) -> [Vertex; 4] {
        let n = self.normal().to_array();
        [(0, 0), (1, 0), (1, 1), (0, 1)].map(|(f, l)| Vertex {
            position: (self.position + self.former * f as f32 + self.latter * l as f32).to_array(), // p, p + self.former, '' + self.latter, '' + former + latter
            color,
            normal: n,
        })
    }

    /// Flat indices for this quad's two triangles, offset by `base`.
    pub fn indices(base: VertexIdx) -> [VertexIdx; 6] {
        [base, base + 1, base + 2, base + 2, base + 3, base]
    }
}

pub struct Voxel {
    pub origin: Vec3,
    pub color: [f32; 3],
}

impl Voxel {
    pub fn new(origin: Vec3, color: [f32; 3]) -> Self {
        Self { origin, color }
    }

    pub fn faces(&self) -> [QuadCCW; 6] {
        let o = self.origin;
        use Vec3 as V;
        [
            (V::ZERO, V::Z, V::Y), // -X
            (V::X, V::Y, V::Z),    // +X
            (V::ZERO, V::X, V::Z), // -Y
            (V::Y, V::Z, V::X),    // +Y
            (V::ZERO, V::Y, V::X), // -Z
            (V::Z, V::X, V::Y),    // +Z
        ]
        .map(|(offset, former, latter)| QuadCCW {
            position: o + offset,
            former,
            latter,
        })
    }

    pub fn vertices(&self) -> Vec<Vertex> {
        self.faces()
            .iter()
            .flat_map(|f| f.vertices(self.color))
            .collect()
    }

    pub fn indices(&self, base: VertexIdx) -> Vec<VertexIdx> {
        (0..6)
            .flat_map(|i| QuadCCW::indices(base + i * 4))
            .collect()
    }
}
