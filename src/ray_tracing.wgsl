@group(0)
@binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;

struct Camera {
    position: vec4<f32>,
    forward: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    fov: f32,
    min_distance: f32,
    max_distance: f32,
}

@group(1)
@binding(0)
var<uniform> camera: Camera;

struct HyperSphere {
    center: vec4<f32>,
    radius: f32,
}

struct HyperSpheres {
    count: u32,
    data: array<HyperSphere>,
}

@group(2)
@binding(0)
var<storage, read> hyper_spheres: HyperSpheres;

struct HyperCuboid {
    center: vec4<f32>,
    size: vec4<f32>,
}

struct HyperCuboids {
    count: u32,
    data: array<HyperCuboid>,
}

@group(2)
@binding(1)
var<storage, read> hyper_cuboids: HyperCuboids;

struct Ray {
    origin: vec4<f32>,
    direction: vec4<f32>,
}

struct Hit {
    hit: bool,
    distance: f32,
    position: vec4<f32>,
    normal: vec4<f32>,
}

fn intersect_hyper_sphere(ray: Ray, hyper_sphere: HyperSphere) -> Hit {
    var hit: Hit;
    hit.hit = false;

    let oc = ray.origin - hyper_sphere.center;
    let a = dot(ray.direction, ray.direction);
    let half_b = dot(oc, ray.direction);
    let c = dot(oc, oc) - hyper_sphere.radius * hyper_sphere.radius;
    let discriminant = half_b * half_b - a * c;

    if discriminant < 0.0 {
        return hit;
    }

    let sqrt_discriminant = sqrt(discriminant);
    let t0 = (-half_b - sqrt_discriminant) / a;
    let t1 = (-half_b + sqrt_discriminant) / a;

    if max(t0, t1) < camera.min_distance || camera.max_distance < min(t0, t1) {
        return hit;
    }

    if t0 > camera.min_distance {
        hit.distance = t0;
    } else {
        hit.distance = t1;
    }

    hit.position = ray.origin + ray.direction * hit.distance;
    hit.normal = normalize(hit.position - hyper_sphere.center);
    if dot(hit.normal, ray.origin - hit.position) < 0.0 {
        hit.normal *= -1.0;
    }

    hit.hit = true;
    return hit;
}

fn intersect_hyper_cuboid(ray: Ray, hyper_cuboid: HyperCuboid) -> Hit {
    var hit: Hit;
    hit.hit = false;

    // TODO: math

    return hit;
}

@compute
@workgroup_size(16, 16)
fn ray_trace(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let size = textureDimensions(output_texture);
    let coords = vec2<i32>(global_id.xy);

    if coords.x >= size.x || coords.y >= size.y {
        return;
    }

    let uv = vec2<f32>(coords) / vec2<f32>(size);
    let normalized_uv = vec2<f32>(uv.x, 1.0 - uv.y) * 2.0 - 1.0;
    let aspect = f32(size.x) / f32(size.y);

    let theta = tan(camera.fov / 2.0);

    var ray: Ray;
    ray.origin = camera.position;
    ray.direction = normalize(
        camera.right * (normalized_uv.x * aspect * theta) + camera.up * (normalized_uv.y * theta) + camera.forward,
    );

    var closest_hit: Hit;
    closest_hit.hit = false;
    closest_hit.distance = camera.max_distance;

    // Check hyper spheres
    for (var i = 0u; i < hyper_spheres.count; i += 1u) {
        let hit = intersect_hyper_sphere(ray, hyper_spheres.data[i]);
        if hit.hit && hit.distance < closest_hit.distance {
            closest_hit = hit;
        }
    }
    // Check hyper cuboids
    for (var i = 0u; i < hyper_cuboids.count; i += 1u) {
        let hit = intersect_hyper_cuboid(ray, hyper_cuboids.data[i]);
        if hit.hit && hit.distance < closest_hit.distance {
            closest_hit = hit;
        }
    }

    var color: vec3<f32>;
    if closest_hit.hit {
        color = normalize(closest_hit.normal.xyz) * 0.5 + 0.5;
    } else {
        color = vec3<f32>(0.0);
    }

    textureStore(output_texture, coords.xy, vec4<f32>(color, 1.0));
}
