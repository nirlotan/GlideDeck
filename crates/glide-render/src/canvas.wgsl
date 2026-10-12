// GlideDeck canvas shaders.
//
// Meshes are stored in element-local space. The per-instance affine maps
// local coordinates straight to clip space; it is computed on the CPU in f64
// relative to the camera, so precision holds at any zoom depth.

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) col_a: vec2<f32>,
    @location(3) col_b: vec2<f32>,
    @location(4) translate: vec2<f32>,
    @location(5) opacity: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@vertex
fn vs_mesh(v: VsIn) -> VsOut {
    var out: VsOut;
    let p = v.col_a * v.pos.x + v.col_b * v.pos.y + v.translate;
    out.clip = vec4<f32>(p, 0.0, 1.0);
    let a = v.color.a * v.opacity;
    // Premultiplied, linear color.
    out.color = vec4<f32>(srgb_to_linear(v.color.rgb) * a, a);
    return out;
}

@fragment
fn fs_mesh(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}

// ---------------------------------------------------------------------------
// Adaptive dot grid, drawn as a fullscreen triangle.

struct Grid {
    inv_a: vec2<f32>,
    inv_b: vec2<f32>,
    origin: vec2<f32>,
    half_viewport: vec2<f32>,
    fine: f32,
    coarse: f32,
    fine_alpha: f32,
    px_per_world: f32,
    color: vec4<f32>,
};

@group(0) @binding(2) var<uniform> grid: Grid;

@vertex
fn vs_grid(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn dots(w: vec2<f32>, spacing: f32, radius: f32) -> f32 {
    let g = w / spacing;
    let d = abs(g - round(g)) * spacing * grid.px_per_world;
    return 1.0 - smoothstep(radius - 0.75, radius + 0.75, length(d));
}

@fragment
fn fs_grid(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let rel = frag.xy - grid.half_viewport;
    let w = grid.inv_a * rel.x + grid.inv_b * rel.y + grid.origin;
    let fine = dots(w, grid.fine, 1.0) * grid.fine_alpha;
    let coarse = dots(w, grid.coarse, 1.5);
    let a = grid.color.a * max(fine, coarse);
    return vec4<f32>(srgb_to_linear(grid.color.rgb) * a, a);
}

// ---------------------------------------------------------------------------
// Images: a unit quad centered on the origin, scaled to the image size by the
// instance affine. Textures are sRGB with straight alpha.

@group(0) @binding(0) var img_tex: texture_2d<f32>;
@group(0) @binding(1) var img_sampler: sampler;

struct ImgIn {
    @builtin(vertex_index) vi: u32,
    @location(2) col_a: vec2<f32>,
    @location(3) col_b: vec2<f32>,
    @location(4) translate: vec2<f32>,
    @location(5) opacity: f32,
    // x: width / height, y: corner radius / height.
    @location(6) params: vec4<f32>,
};

struct ImgOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
    @location(2) params: vec4<f32>,
};

@vertex
fn vs_image(v: ImgIn) -> ImgOut {
    let uv = vec2<f32>(f32(v.vi & 1u), f32((v.vi >> 1u) & 1u));
    let local = uv - vec2<f32>(0.5, 0.5);
    var out: ImgOut;
    out.clip = vec4<f32>(v.col_a * local.x + v.col_b * local.y + v.translate, 0.0, 1.0);
    out.uv = uv;
    out.opacity = v.opacity;
    out.params = v.params;
    return out;
}

fn rounded_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_image(in: ImgOut) -> @location(0) vec4<f32> {
    let c = textureSample(img_tex, img_sampler, in.uv);
    var mask = 1.0;
    let aspect = in.params.x;
    let r = in.params.y;
    if (r > 0.0) {
        let p = (in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
        let d = rounded_box(p, vec2<f32>(aspect * 0.5, 0.5), min(r, min(aspect, 1.0) * 0.5));
        let aa = max(fwidth(d), 1e-6);
        mask = 1.0 - smoothstep(-aa, aa, d);
    }
    let a = c.a * in.opacity * mask;
    return vec4<f32>(c.rgb * a, a);
}

// ---------------------------------------------------------------------------
// Soft shadows: an analytic signed-distance falloff around a rounded box or
// ellipse, drawn beneath the element. No blur passes needed.

struct ShadowIn {
    @builtin(vertex_index) vi: u32,
    @location(0) col_a: vec2<f32>,
    @location(1) col_b: vec2<f32>,
    @location(2) translate: vec2<f32>,
    @location(3) half: vec2<f32>,
    // x: corner radius, y: blur, z: 1 for ellipse.
    @location(4) params: vec4<f32>,
    @location(5) offset: vec2<f32>,
    @location(6) color: vec4<f32>,
};

struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) half: vec2<f32>,
    @location(2) params: vec4<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_shadow(v: ShadowIn) -> ShadowOut {
    let uv = vec2<f32>(f32(v.vi & 1u), f32((v.vi >> 1u) & 1u));
    let reach = v.half + vec2<f32>(v.params.y * 2.5);
    let p = (uv * 2.0 - vec2<f32>(1.0)) * reach;
    let local = p + v.offset;
    var out: ShadowOut;
    out.clip = vec4<f32>(v.col_a * local.x + v.col_b * local.y + v.translate, 0.0, 1.0);
    out.p = p;
    out.half = v.half;
    out.params = v.params;
    out.color = v.color;
    return out;
}

@fragment
fn fs_shadow(in: ShadowOut) -> @location(0) vec4<f32> {
    let blur = max(in.params.y, 1e-4);
    var d: f32;
    if (in.params.z > 0.5) {
        let k = length(in.p / in.half);
        d = (k - 1.0) * min(in.half.x, in.half.y);
    } else {
        d = rounded_box(in.p, in.half, min(in.params.x, min(in.half.x, in.half.y)));
    }
    // Smooth, Gaussian-like falloff centred on the edge.
    let falloff = 0.5 - 0.5 * tanh(d / (blur * 0.55));
    let a = in.color.a * falloff;
    return vec4<f32>(srgb_to_linear(in.color.rgb) * a, a);
}

// ---------------------------------------------------------------------------
// Vertical background gradient (screen space).

struct Backdrop {
    top: vec4<f32>,
    bottom: vec4<f32>,
    height: f32,
};

@group(0) @binding(3) var<uniform> backdrop: Backdrop;

@fragment
fn fs_backdrop(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let t = clamp(frag.y / max(backdrop.height, 1.0), 0.0, 1.0);
    let c = mix(srgb_to_linear(backdrop.top.rgb), srgb_to_linear(backdrop.bottom.rgb), t);
    return vec4<f32>(c, 1.0);
}
