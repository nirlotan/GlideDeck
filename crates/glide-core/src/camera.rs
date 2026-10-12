use crate::document::TransitionStyle;
use crate::easing::Easing;
use crate::element::Element;
use crate::geom::Aabb;
use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

pub const MIN_ZOOM: f64 = 1e-9;
pub const MAX_ZOOM: f64 = 1e9;

/// A 2D camera over the infinite canvas.
///
/// All math is f64 so the canvas stays precise across extreme zoom depths;
/// conversion to f32 happens only per element, relative to the camera.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    /// World-space point shown at the viewport center.
    pub center: DVec2,
    /// Screen units (points) per world unit.
    pub zoom: f64,
    /// View rotation in radians. A camera rotated by θ shows the world rotated by −θ.
    pub rotation: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { center: DVec2::ZERO, zoom: 1.0, rotation: 0.0 }
    }
}

impl Camera {
    /// World → screen transform for a viewport of the given size (origin top-left).
    pub fn view_affine(&self, viewport: DVec2) -> DAffine2 {
        DAffine2::from_translation(viewport * 0.5)
            * DAffine2::from_mat2(DMat2::from_scale_angle(DVec2::splat(self.zoom), -self.rotation))
            * DAffine2::from_translation(-self.center)
    }

    pub fn world_to_screen(&self, world: DVec2, viewport: DVec2) -> DVec2 {
        let d = DVec2::from_angle(-self.rotation).rotate(world - self.center) * self.zoom;
        viewport * 0.5 + d
    }

    pub fn screen_to_world(&self, screen: DVec2, viewport: DVec2) -> DVec2 {
        let d = (screen - viewport * 0.5) / self.zoom;
        self.center + DVec2::from_angle(self.rotation).rotate(d)
    }

    /// Converts a screen-space displacement into a world-space displacement.
    pub fn screen_delta_to_world(&self, delta: DVec2) -> DVec2 {
        DVec2::from_angle(self.rotation).rotate(delta / self.zoom)
    }

    /// Pans so that content follows a screen-space drag.
    pub fn pan_by_screen(&mut self, delta: DVec2) {
        self.center -= self.screen_delta_to_world(delta);
    }

    /// Zooms by `factor`, keeping the world point under `anchor` fixed on screen.
    pub fn zoom_about(&mut self, anchor: DVec2, factor: f64, viewport: DVec2) {
        let before = self.screen_to_world(anchor, viewport);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(anchor, viewport);
        self.center += before - after;
    }

    /// Rotates the view by `angle` radians, keeping the world point under `anchor` fixed.
    pub fn rotate_about(&mut self, anchor: DVec2, angle: f64, viewport: DVec2) {
        let before = self.screen_to_world(anchor, viewport);
        self.rotation = wrap_angle(self.rotation + angle);
        let after = self.screen_to_world(anchor, viewport);
        self.center += before - after;
    }

    /// Visible world-space bounds (an AABB around the possibly rotated view).
    pub fn visible_world_aabb(&self, viewport: DVec2) -> Aabb {
        Aabb::from_points([
            self.screen_to_world(DVec2::ZERO, viewport),
            self.screen_to_world(DVec2::new(viewport.x, 0.0), viewport),
            self.screen_to_world(viewport, viewport),
            self.screen_to_world(DVec2::new(0.0, viewport.y), viewport),
        ])
    }

    /// The camera that exactly frames a frame element (aligned with its
    /// rotation), leaving `margin` screen units of padding.
    pub fn for_frame(frame: &Element, viewport: DVec2, margin: f64) -> Option<Camera> {
        let size = frame.kind.intrinsic_size()? * frame.transform.scale.abs();
        if size.x <= 0.0 || size.y <= 0.0 {
            return None;
        }
        let avail = (viewport - DVec2::splat(2.0 * margin)).max(DVec2::ONE);
        Some(Camera {
            center: frame.transform.position(),
            zoom: (avail.x / size.x).min(avail.y / size.y).clamp(MIN_ZOOM, MAX_ZOOM),
            rotation: wrap_angle(frame.transform.rotation.to_radians()),
        })
    }

    /// A camera showing `bounds` (an axis-aligned world rect) with the given
    /// rotation, leaving `margin` screen units around it.
    pub fn framing(bounds: Aabb, rotation: f64, viewport: DVec2, margin: f64) -> Camera {
        if bounds.is_empty() {
            return Camera { rotation, ..Camera::default() };
        }
        // Size of the bounds once rotated into view space.
        let corners = [
            bounds.min,
            DVec2::new(bounds.max.x, bounds.min.y),
            bounds.max,
            DVec2::new(bounds.min.x, bounds.max.y),
        ];
        let rot = DVec2::from_angle(-rotation);
        let view = Aabb::from_points(corners.iter().map(|c| rot.rotate(*c - bounds.center())));
        let size = view.size().max(DVec2::splat(1e-9));
        let avail = (viewport - DVec2::splat(2.0 * margin)).max(DVec2::splat(1.0));
        let zoom = (avail.x / size.x).min(avail.y / size.y).clamp(MIN_ZOOM, MAX_ZOOM);
        Camera { center: bounds.center(), zoom, rotation }
    }
}

/// Wraps an angle into `(-π, π]`.
pub fn wrap_angle(a: f64) -> f64 {
    let a = (a + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
}

/// A timed camera move between two views.
///
/// Zoom is interpolated in log space and the center is parameterized by the
/// visible extent (1/zoom), so motion looks uniform on screen even across
/// large zoom changes. The waypoint system builds on this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTween {
    pub from: Camera,
    pub to: Camera,
    pub duration: f64,
    pub easing: Easing,
    pub elapsed: f64,
    pub style: TransitionStyle,
    /// Viewport extent in screen units; the smooth path is defined in terms
    /// of visible world width, so it needs the view size.
    pub view_extent: f64,
}

/// Curvature of the smooth zoom arc. √2 is the "optimal" value from van Wijk
/// & Nuij; a little lower keeps presentations from zooming out too far.
const RHO: f64 = 1.25;

impl CameraTween {
    pub fn new(from: Camera, to: Camera, duration: f64, easing: Easing) -> Self {
        CameraTween {
            from,
            to,
            duration: duration.max(1e-6),
            easing,
            elapsed: 0.0,
            style: TransitionStyle::Direct,
            view_extent: 1.0,
        }
    }

    pub fn with_style(mut self, style: TransitionStyle, viewport: DVec2) -> Self {
        self.style = style;
        self.view_extent = viewport.max_element().max(1.0);
        if style == TransitionStyle::Cut {
            self.duration = 1e-6;
        }
        self
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// Advances by `dt` seconds and returns the camera at the new time.
    pub fn step(&mut self, dt: f64) -> Camera {
        self.elapsed = (self.elapsed + dt).min(self.duration);
        self.sample(self.elapsed / self.duration)
    }

    pub fn sample(&self, t: f64) -> Camera {
        let u = self.easing.apply(t);
        if u >= 1.0 {
            return self.to;
        }
        match self.style {
            TransitionStyle::Cut => self.to,
            TransitionStyle::Smooth => self.sample_smooth(u),
            TransitionStyle::Direct => self.sample_direct(u),
        }
    }

    /// van Wijk & Nuij, "Smooth and efficient zooming and panning" (2003).
    fn sample_smooth(&self, u: f64) -> Camera {
        let (a, b) = (self.from, self.to);
        let w0 = self.view_extent / a.zoom;
        let w1 = self.view_extent / b.zoom;
        let delta = b.center - a.center;
        let dist = delta.length();
        let rotation = a.rotation + wrap_angle(b.rotation - a.rotation) * u;
        let rho2 = RHO * RHO;
        if dist < 1e-9 * w0.max(w1) {
            // Pure zoom: exponential in width.
            let w = w0 * (w1 / w0).powf(u);
            return Camera { center: a.center + delta * u, zoom: self.view_extent / w, rotation };
        }
        let b0 = (w1 * w1 - w0 * w0 + rho2 * rho2 * dist * dist) / (2.0 * w0 * rho2 * dist);
        let b1 = (w1 * w1 - w0 * w0 - rho2 * rho2 * dist * dist) / (2.0 * w1 * rho2 * dist);
        // r = ln(-b + sqrt(b² + 1)) = -asinh(b), written stably.
        let r0 = -b0.asinh();
        let r1 = -b1.asinh();
        let s = u * (r1 - r0) / RHO;
        let along = w0 / rho2 * (r0.cosh() * (RHO * s + r0).tanh() - r0.sinh());
        let w = w0 * r0.cosh() / (RHO * s + r0).cosh();
        Camera {
            center: a.center + delta * (along / dist),
            zoom: (self.view_extent / w).clamp(MIN_ZOOM, MAX_ZOOM),
            rotation,
        }
    }

    fn sample_direct(&self, u: f64) -> Camera {
        let (a, b) = (self.from, self.to);
        let ln_zoom = a.zoom.ln() + (b.zoom.ln() - a.zoom.ln()) * u;
        let zoom = ln_zoom.exp();
        let inv_a = 1.0 / a.zoom;
        let inv_b = 1.0 / b.zoom;
        let w = if (inv_b - inv_a).abs() > 1e-12 * inv_a.max(inv_b) {
            ((1.0 / zoom) - inv_a) / (inv_b - inv_a)
        } else {
            u
        };
        let center = a.center + (b.center - a.center) * w;
        let rotation = a.rotation + wrap_angle(b.rotation - a.rotation) * u;
        Camera { center, zoom, rotation }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: DVec2, b: DVec2) -> bool {
        (a - b).length() < 1e-6
    }

    #[test]
    fn screen_world_roundtrip() {
        let cam = Camera { center: DVec2::new(1e7, -3.0), zoom: 0.37, rotation: 0.8 };
        let vp = DVec2::new(1280.0, 720.0);
        let p = DVec2::new(1e7 + 12.5, 44.0);
        let s = cam.world_to_screen(p, vp);
        assert!(approx(cam.screen_to_world(s, vp), p));
        assert!(approx(cam.view_affine(vp).transform_point2(p), s));
    }

    #[test]
    fn zoom_and_rotate_keep_anchor() {
        let vp = DVec2::new(800.0, 600.0);
        let mut cam = Camera { center: DVec2::new(5.0, 5.0), zoom: 2.0, rotation: 0.3 };
        let anchor = DVec2::new(100.0, 450.0);
        let w = cam.screen_to_world(anchor, vp);
        cam.zoom_about(anchor, 7.5, vp);
        assert!(approx(cam.world_to_screen(w, vp), anchor));
        cam.rotate_about(anchor, 1.1, vp);
        assert!(approx(cam.world_to_screen(w, vp), anchor));
    }

    #[test]
    fn smooth_path_zooms_out_between_distant_views() {
        let a = Camera { center: DVec2::ZERO, zoom: 1.0, rotation: 0.0 };
        let b = Camera { center: DVec2::new(10_000.0, 0.0), zoom: 1.0, rotation: 0.0 };
        let vp = DVec2::new(1000.0, 600.0);
        let tw = CameraTween::new(a, b, 1.0, Easing::Linear).with_style(TransitionStyle::Smooth, vp);
        let mid = tw.sample(0.5);
        assert!(mid.zoom < 0.5, "should pull back mid-flight, got {}", mid.zoom);
        assert!((mid.center.x - 5000.0).abs() < 1.0);
        let start = tw.sample(0.0);
        assert!((start.zoom - 1.0).abs() < 1e-6 && start.center.length() < 1e-6);
        let end = tw.sample(1.0);
        assert_eq!(end, b);
        // Nearly-coincident centers fall back to a pure zoom.
        let c = Camera { zoom: 8.0, ..a };
        let z = CameraTween::new(a, c, 1.0, Easing::Linear).with_style(TransitionStyle::Smooth, vp).sample(0.5);
        assert!((z.zoom - 8f64.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn tween_hits_endpoints() {
        let a = Camera { center: DVec2::ZERO, zoom: 1.0, rotation: 0.0 };
        let b = Camera { center: DVec2::new(100.0, 50.0), zoom: 20.0, rotation: 3.0 };
        let tw = CameraTween::new(a, b, 1.0, Easing::EaseInOutCubic);
        let s0 = tw.sample(0.0);
        let s1 = tw.sample(1.0);
        assert!(approx(s0.center, a.center) && (s0.zoom - a.zoom).abs() < 1e-9);
        assert!(approx(s1.center, b.center) && (s1.zoom - b.zoom).abs() < 1e-9);
        assert!((s1.rotation - 3.0).abs() < 1e-9);
    }
}
