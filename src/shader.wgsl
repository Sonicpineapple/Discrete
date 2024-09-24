@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage,read> group: array<i32>;
@group(0) @binding(2) var<storage,read> sticker: array<i32>;


struct Params {
    mirrors: mat4x4<f32>,
    edges: vec4<u32>,
    point: vec4<f32>,
    cut_circle: vec4<f32>,
    scale: vec2<f32>,
    col_scale: f32,
    depth: u32,
    flags: u32,
    mirror_count: u32,
}

fn reflect(c: vec4<f32>, p: vec4<f32>) -> vec4<f32> {
    let s = dot(c * p, vec4(-1.0, 1.0, 1.0, 1.0));
    let xy = c.x * p.y - c.y * p.x;
    let xz = c.x * p.z - c.z * p.x;
    let xw = c.x * p.w - c.w * p.x;
    let yz = c.y * p.z - c.z * p.y;
    let yw = c.y * p.w - c.w * p.y;
    let zw = c.z * p.w - c.w * p.z;

    return -s * c - vec4(
        dot(vec3(xy, xz, xw), c.yzw),
        dot(vec3(xy, yz, yw), c.xzw),
        dot(vec3(xz, -yz, zw), c.xyw),
        dot(vec3(xw, -yw, -zw), c.xyz),
    );
}

fn in_circle(c: vec4<f32>, p: vec4<f32>) -> bool {
    return -dot(c * p,vec4(-1.0, 1.0, 1.0, 1.0)) >= 0;
}

fn how_in_circle(c: vec4<f32>, p: vec4<f32>) -> f32 {
    return -dot(c * p,vec4(-1.0, 1.0, 1.0, 1.0));
}

fn up(xy: vec2<f32>) -> vec4<f32> {
    let ni = 0.5 * dot(xy, xy);
    return vec4(ni + 0.5, ni - 0.5, xy);
}

fn down(p: vec4<f32>) -> vec2<f32> {
    return p.zw / (p.x - p.y);
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(in.position, 0.0, 1.0);
    out.color = vec4(in.position * params.scale, 0.0, 1.0);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var p = up(in.color.xy);
    var q = params.point;

    var elem = 0;
    var k = 0;
    for (var i: u32 = 0u; i < params.depth; i++) {
        var done = true;
        for (var j: u32 = 0u; j < params.mirror_count; j++) {
            if !in_circle(params.mirrors[j],p) {
                p = reflect(params.mirrors[j],p);
                elem = mul_elem_gen(elem,j);
                done = false;
                k += 1;
            }
        }
        if done {
            break;
        }
    }

    if (params.flags & 1) > 0 && k == 0 {
        return vec4(0.5,0.5,0.5,1.);
    }

    if (params.flags & 2) == 0 || elem == -1 {
        var dist = params.col_scale;
        for (var i = 0u; i < params.mirror_count; i++) {
            if params.edges[i] > 0u {
                dist = min(dist,how_in_circle(params.mirrors[i],p));
            }
        }
        return turbo(dist,0.,params.col_scale);
    }

    if in_circle(params.cut_circle,p) {
        elem = get_sticker(elem,1u);
        // return vec4(0.2,0.4,0.6,1.);
    }

    if (params.flags & 4) > 0 {
        elem = mul_elem_gen(elem,params.mirror_count-1);
    }
    return turbo(f32(get_col(elem)) / 50.,0.,params.col_scale);
    // return turbo(f32(elem) / 20.,0.,params.col_scale);
}

/// Get the colour of where we started by inverting the element that gets us home
fn get_col(elem: i32) -> i32 {
    return group[(params.mirror_count + 1) * u32(elem)];
}

/// Apply a generator to an element
fn mul_elem_gen(elem: i32, gen: u32) -> i32 {
    if elem == -1 {
        return elem;
    }
    return group[(params.mirror_count + 1) * u32(elem) + gen + 1];
}

fn get_sticker(elem: i32, cut_inclusion: u32) -> i32 {
    if elem == -1 {
        return elem;
    }
    return sticker[u32(elem) * 2 + cut_inclusion];
}

fn turbo(value: f32, min: f32, max: f32) -> vec4<f32> {
    let kRedVec4: vec4<f32> = vec4(0.13572138, 4.61539260, -42.66032258, 132.13108234);
    let kGreenVec4: vec4<f32> = vec4(0.09140261, 2.19418839, 4.84296658, -14.18503333);
    let kBlueVec4: vec4<f32> = vec4(0.10667330, 12.64194608, -60.58204836, 110.36276771);
    let kRedVec2: vec2<f32> = vec2(-152.94239396, 59.28637943);
    let kGreenVec2: vec2<f32> = vec2(4.27729857, 2.82956604);
    let kBlueVec2: vec2<f32> = vec2(-89.90310912, 27.34824973);

    let x = saturate((value - min) / (max - min));
    if abs(x) < 0.51 && abs(x) > 0.49 {
        return vec4(1.0, 1.0, 1.0, 1.0);
    }
    let v4: vec4<f32> = vec4( 1.0, x, x * x, x * x * x);
    let v2: vec2<f32> = v4.zw * v4.z;
    return vec4(
        dot(v4, kRedVec4)   + dot(v2, kRedVec2),
        dot(v4, kGreenVec4) + dot(v2, kGreenVec2),
        dot(v4, kBlueVec4)  + dot(v2, kBlueVec2),
        1.0,
    );
}