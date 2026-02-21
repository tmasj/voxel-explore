use ash::vk;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
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

type GeometryDataIndex = u16; // Can be u16 or u32 

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
