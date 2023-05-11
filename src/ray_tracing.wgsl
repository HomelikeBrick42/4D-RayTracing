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
    bounce_count: u32,
    sample_count: u32,
}

@group(1)
@binding(0)
var<uniform> camera: Camera;

struct HyperSphere {
    center: vec4<f32>,
    radius: f32,
    material: u32,
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
    material: u32,
}

struct HyperCuboids {
    count: u32,
    data: array<HyperCuboid>,
}

@group(2)
@binding(1)
var<storage, read> hyper_cuboids: HyperCuboids;

struct Material {
    base_color: vec3<f32>,
    emissive_color: vec3<f32>,
    emission_strength: f32,
}

struct Materials {
    count: u32,
    data: array<Material>,
}

@group(3)
@binding(0)
var<storage, read> materials: Materials;

struct Ray {
    origin: vec4<f32>,
    direction: vec4<f32>,
}

struct Hit {
    hit: bool,
    distance: f32,
    position: vec4<f32>,
    normal: vec4<f32>,
    material: u32,
}

fn intersect_hyper_sphere(ray: Ray, hyper_sphere: HyperSphere) -> Hit {
    var hit: Hit;
    hit.hit = false;
    hit.material = hyper_sphere.material;

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

    if t0 > camera.min_distance {
        hit.distance = t0;
    } else {
        hit.distance = t1;
    }

    if hit.distance < camera.min_distance || camera.max_distance < hit.distance {
        return hit;
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
    hit.material = hyper_cuboid.material;

    // TODO: math

    return hit;
}

fn random_value(state: ptr<function, u32>) -> f32 {
    *state = *state * 747796405u + 2891336453u;
    var result = ((*state >> ((*state >> 28u) + 4u)) ^ *state) * 277803737u;
    result = (result >> 22u) ^ result;
    return f32(result) / 4294967295.0;
}

fn random_value_normal_distribution(state: ptr<function, u32>) -> f32 {
    let theta = 2.0 * 3.1415926 * random_value(state);
    let rho = sqrt(-2.0 * log(random_value(state)));
    return rho * cos(theta);
}

fn random_direction(state: ptr<function, u32>) -> vec4<f32> {
    return normalize(vec4<f32>(
        random_value_normal_distribution(state),
        random_value_normal_distribution(state),
        random_value_normal_distribution(state),
        random_value_normal_distribution(state),
    ));
}

fn random_direction_in_hemisphere(state: ptr<function, u32>, normal: vec4<f32>) -> vec4<f32> {
    var direction = random_direction(state);
    if dot(direction, normal) < 0.0 {
        direction *= -1.0;
    }
    return direction;
}

fn get_closest_hit(ray: Ray) -> Hit {
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

    return closest_hit;
}

fn trace(ray: Ray, state: ptr<function, u32>) -> vec3<f32> {
    var ray = ray;
    var incoming_light = vec3<f32>(0.0);
    var ray_color = vec3<f32>(1.0);

    for (var i = 0u; i < camera.bounce_count; i += 1u) {
        let hit = get_closest_hit(ray);
        if hit.hit {
            let material = materials.data[hit.material];

            ray.origin = hit.position + hit.normal * camera.min_distance;
            ray.direction = normalize(hit.normal + random_direction(state));

            incoming_light += (material.emissive_color * material.emission_strength) * ray_color;
            ray_color *= material.base_color;
        } else {
            let up_color = vec3<f32>(0.3, 0.4, 0.8);
            let down_color = vec3<f32>(0.2, 0.2, 0.2);
            incoming_light += mix(down_color, up_color, ray.direction.y * 0.5 + 0.5) * ray_color;
            break;
        }
    }

    return incoming_light;
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

    var state: u32 = u32(coords.x + coords.y * size.x);

    let uv = vec2<f32>(coords) / vec2<f32>(size);
    let normalized_uv = vec2<f32>(uv.x, 1.0 - uv.y) * 2.0 - 1.0;
    let aspect = f32(size.x) / f32(size.y);

    let theta = tan(camera.fov / 2.0);

    var ray: Ray;
    ray.origin = camera.position;
    ray.direction = normalize(
        camera.right * (normalized_uv.x * aspect * theta) + camera.up * (normalized_uv.y * theta) + camera.forward,
    );

    var color = vec3<f32>(0.0);
    for (var i = 0u; i < camera.sample_count; i += 1u) {
        color += trace(ray, &state);
    }
    color /= f32(camera.sample_count);
    textureStore(output_texture, coords.xy, vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
