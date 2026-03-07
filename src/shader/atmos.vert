#version 450
#extension GL_EXT_debug_printf : enable

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;



layout(location = 0) out vec3 worldsp_ray;

const vec2 positions[6] = vec2[](
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2(-1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2(-1.0,  1.0)
);

void main() {
    vec2 pos = positions[gl_VertexIndex];
    gl_Position = vec4(pos, 0.9999, 1.0);

    // NDC -> view space direction
    vec4 view_ray = inverse(ubo.proj) * vec4(pos, 1.0, 1.0);
    view_ray /= view_ray.w;  // perspective divide

    // view space -> world space (rotation only, no translation)
    worldsp_ray = mat3(inverse(ubo.view)) * view_ray.xyz;
}