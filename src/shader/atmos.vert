#version 450
#extension GL_EXT_debug_printf : enable

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) out vec3 fragColor;

void main() {
    debugPrintfEXT("Hello from vertex %d!\n", gl_VertexIndex);
    fragColor = vec3(0,.5,.5);
}