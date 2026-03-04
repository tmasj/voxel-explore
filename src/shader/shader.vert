#version 450
#extension GL_EXT_debug_printf : enable


layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;
layout(location = 1) out vec3 fragPos;

void main() {
    //debugPrintfEXT("Hello from vertex %d!\n", gl_VertexIndex);
    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(inPosition, 1.0);
    fragColor = inColor;
    fragColor.r *= 0;
    fragPos = vec3(ubo.model * vec4(inPosition, 1.0));
}