#version 450
layout(location = 0) in vec3 fragColor;
layout(location = 1) in vec3 fragPos;

layout(location = 0) out vec4 outColor;

void main() {
    vec3 normal = normalize(cross(dFdx(fragPos), dFdy(fragPos)));
    vec3 lightDir = -normalize(vec3(1.0, 1.0, 1.0));
    float intensity = max(dot(normal, lightDir), 0.20);
    outColor = vec4(abs(fragColor) * intensity, 1.0);
}