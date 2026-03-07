#version 450
layout(location = 0) in vec3 worldsp_ray;

layout(location = 0) out vec4 outColor;

void main() {
    vec3 lightDir = normalize(vec3(1.0, 1.0, 1.0));
    float blueness = dot(lightDir,normalize(worldsp_ray));
    outColor = vec4(0.,0., blueness ,1.0);
}