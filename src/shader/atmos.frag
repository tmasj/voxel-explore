#version 450
layout(location = 0) in vec3 worldsp_ray;
layout(location = 1) in vec3 cam_pos;


layout(location = 0) out vec4 outColor;

const float THICKNESS = 0.1;
const float MIN_RENDER_DIST = 0.3;
const float MAX_RENDER_DIST = 200.0;

struct AxisT {
    float numer;
    float denom;
    float t;
    vec2 cam_perp;
    vec2 ray_perp;
};

// package private
AxisT axis_t(vec2 cam_perp, vec2 ray_perp) {
    AxisT at;
    at.cam_perp = cam_perp;
    at.ray_perp = ray_perp;
    at.numer = dot(cam_perp, ray_perp);
    at.denom = dot(ray_perp, ray_perp);
    at.t = -at.numer / at.denom;
    return at;
}

bool axis_t_poor_conditioning(AxisT at) {
    return at.numer >= MAX_RENDER_DIST || at.denom <= 0.00001;
}

vec3 axis_closest(AxisT at, vec3 ray) {
    //! The point closest to the axis, in world space
    return cam_pos + at.t * ray;
}

bool axis_hit(AxisT at, float along_axis) {
    if (axis_t_poor_conditioning(at)) return false;
    if (at.t <= MIN_RENDER_DIST) return false;
    float dist = length(at.cam_perp + at.t * at.ray_perp);
    float tick = 1.0 - step(0.05, abs(along_axis - round(along_axis)));
    return dist <= THICKNESS * (1.0 + 2.0 * tick);
}

// public
bool x_axis_hit(vec3 ray) {
    AxisT at = axis_t(cam_pos.yz, ray.yz);
    return axis_hit(at, axis_closest(at, ray).x);
}

bool y_axis_hit(vec3 ray) {
    AxisT at = axis_t(cam_pos.xz, ray.xz);
    return axis_hit(at, axis_closest(at, ray).y);
}

bool z_axis_hit(vec3 ray) {
    AxisT at = axis_t(cam_pos.xy, ray.xy);
    return axis_hit(at, axis_closest(at, ray).z);
}

void main() {
    vec3 ray = normalize(worldsp_ray);
    vec3 lightDir = normalize(vec3(1.0, 1.0, 1.0));
    float blueness = 0.8 + max(0.1*dot(lightDir,ray),0.);
    float redness = 0.8 + max(0.1*dot(lightDir,-ray),0.); 
    outColor = vec4(redness,0.9, blueness ,1.0);

    bool xah = x_axis_hit(ray);
    bool yah = y_axis_hit(ray);
    bool zah = z_axis_hit(ray);
    if (xah || yah || zah) {
        outColor = vec4(0.,0.,0.,1.);
    }
    outColor = vec4(outColor.x + float(xah), outColor.y + float(yah), outColor.z + float(zah), 1.0);

}

