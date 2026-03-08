#version 450
layout(location = 0) in vec3 worldsp_ray;
layout(location = 1) in vec3 cam_pos;


layout(location = 0) out vec4 outColor;


float sigmoid(float x) { return 2/(1 + exp2(-x)) - 1; } 

void main() {
    vec3 ray = normalize(worldsp_ray);
    vec3 lightDir = normalize(vec3(1.0, 1.0, 1.0));
    float blueness = 0.8 + max(0.1*dot(lightDir,ray),0.);
    float redness = 0.8 + max(0.1*dot(lightDir,-ray),0.); 
    outColor = vec4(redness,0.9, blueness ,1.0);

    float numer = dot(cam_pos.yz, ray.yz);
    float denom = dot(ray.yz, ray.yz);
    if (numer < 2000000. && denom > 0.00001) { // not too far and not parallel to X axis
        float thickness = 0.1;
        float render_dist = 100.;
        float min_render_dist = 0.3;
        float t_x = -numer / denom;
        vec3 closest = cam_pos + t_x * ray;
        float dist_to_xaxis = length(closest.yz);
        if ( t_x > min_render_dist && dist_to_xaxis <= thickness) {
            outColor = vec4(1.0, 1.0, 1.0, 1.0);
        }
    }

}

