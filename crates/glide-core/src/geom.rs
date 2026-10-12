use glam::DVec2;

/// Axis-aligned bounding box in world (or screen) space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: DVec2,
    pub max: DVec2,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: DVec2::splat(f64::INFINITY),
        max: DVec2::splat(f64::NEG_INFINITY),
    };

    pub fn from_points(points: impl IntoIterator<Item = DVec2>) -> Self {
        points.into_iter().fold(Self::EMPTY, |acc, p| acc.including(p))
    }

    pub fn from_center_half(center: DVec2, half: DVec2) -> Self {
        Aabb { min: center - half, max: center + half }
    }

    pub fn including(self, p: DVec2) -> Self {
        Aabb { min: self.min.min(p), max: self.max.max(p) }
    }

    pub fn union(self, o: Aabb) -> Self {
        Aabb { min: self.min.min(o.min), max: self.max.max(o.max) }
    }

    pub fn is_empty(&self) -> bool {
        !(self.min.x <= self.max.x && self.min.y <= self.max.y)
    }

    pub fn size(&self) -> DVec2 {
        self.max - self.min
    }

    pub fn center(&self) -> DVec2 {
        (self.min + self.max) * 0.5
    }

    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x <= o.max.x && o.min.x <= self.max.x && self.min.y <= o.max.y && o.min.y <= self.max.y
    }

    pub fn contains_aabb(&self, o: &Aabb) -> bool {
        self.min.x <= o.min.x && self.min.y <= o.min.y && self.max.x >= o.max.x && self.max.y >= o.max.y
    }

    pub fn contains(&self, p: DVec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}
