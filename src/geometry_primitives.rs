use ash::vk;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    // Could add texture coords, ambient occlusion, etc.
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl Vertex {
    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
    }
    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
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

        return [first, second];
    }
}

fn get_triangle_geometry() -> Vec<Vertex> {
    vec![
        Vertex {
            position: [0.0, -0.5, 0.0],
            color: [1.0, 0.0, 0.2],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            color: [0.1, 0.5, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.0],
            color: [0.7, 0.5, 0.0],
        },
    ]
}

fn get_triangle_geometry2() -> Vec<Vertex> {
    vec![
        Vertex {
            position: [0.5, -0.5, 0.0],
            color: [1.0, 0.0, 0.2],
        },
        Vertex {
            position: [0.0, 0.5, 0.0],
            color: [0.1, 0.5, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            color: [0.7, 0.5, 0.0],
        },
    ]
}

pub type GeometryDataIndex = u16; // Can be u16 or u32 
pub const GeometryDataIndexVkType: vk::IndexType = vk::IndexType::UINT16;

pub struct IndexedVertexGeometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<GeometryDataIndex>,
}

pub fn triangle_vertices_indexed() -> Vec<Vertex> {
    vec![
        Vertex {
            position: [-0.5, -0.5, 0.0],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.0],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            color: [0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.0],
            color: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [-0.5, -0.5, -0.5],
            color: [1.0, 0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            color: [0.0, 1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            color: [0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            color: [1.0, 1.0, 1.0],
        },
    ]
}

pub fn triangle_geom_indices() -> Vec<GeometryDataIndex> {
    vec![0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4]
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct UniformBufferObject {
    pub model: glam::Mat4,
    pub view: glam::Mat4, // 64 bytes
    pub proj: glam::Mat4, // 64 bytes
}

pub struct Voxel(pub IndexedVertexGeometry);

impl Voxel {
    pub fn new(pos: Vertex) -> Self {
        let mut allv: Vec<Vertex> = vec![];
        for xsig in [0, 1] {
            for ysig in [0, 1] {
                for zsig in [0, 1] {
                    allv.push(Vertex {
                        position: [
                            pos.position[0] + (xsig as f32),
                            pos.position[1] + (ysig as f32),
                            pos.position[2] + (zsig as f32),
                        ],
                        color: pos.color,
                    });
                }
            }
        }
        let mut indices = vec![];
        // Each face: (fixed_axis, fixed_value, quad indices in CCW order when viewed from outside)
        let faces: [(GeometryDataIndex, GeometryDataIndex, [GeometryDataIndex; 4]); 6] = [
            (0, 0, [0, 2, 6, 4]), // -X face, CCW from outside (looking in +X)
            (0, 1, [1, 5, 7, 3]), // +X face, CCW from outside (looking in -X)
            (1, 0, [0, 4, 5, 1]), // -Y
            (1, 1, [2, 3, 7, 6]), // +Y
            (2, 0, [0, 1, 3, 2]), // -Z
            (2, 1, [4, 6, 7, 5]), // +Z
        ];

        for (_, _, quad) in &faces {
            let [a, b, c, d] = *quad;
            indices.extend_from_slice(&[a, b, c, c, d, a]); // two triangles
        }

        return Self(IndexedVertexGeometry {
            vertices: allv,
            indices,
        });
    }
}
