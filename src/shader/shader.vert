#version 450
#extension GL_EXT_debug_printf : enable


layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;
layout(location = 2) in vec3 inNorm;
layout(location = 3) in uint instanceRelPos;

layout(location = 0) out vec3 fragColor;
layout(location = 1) out vec3 fragPos;
layout(location = 2) out flat vec3 outNorm;

void main() {
    uint instx = uint((instanceRelPos >> 11) & 0x1F);
    uint insty = uint((instanceRelPos >> 6)  & 0x1F);
    uint instz = uint((instanceRelPos >> 1)  & 0x1F);
    vec3 pos = inPosition + vec3(instx,insty,instz);
    //debugPrintfEXT("Hello from instance at %d %d %d!\n", instx, insty, instz);
    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(pos, 1.0);
    fragColor = inColor;
    fragColor.r *= 0;
    fragPos = vec3(ubo.model * vec4(pos, 1.0));
    outNorm = inNorm;
}