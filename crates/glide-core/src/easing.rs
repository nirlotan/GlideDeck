use serde::{Deserialize, Serialize};

/// Easing curves for camera motion. `t` is normalized time in `[0, 1]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    Linear,
    EaseInOutSine,
    #[default]
    EaseInOutCubic,
    EaseInOutQuint,
    EaseOutExpo,
}

impl Easing {
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseInOutSine => -((std::f64::consts::PI * t).cos() - 1.0) / 2.0,
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::EaseInOutQuint => {
                if t < 0.5 {
                    16.0 * t.powi(5)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
                }
            }
            Easing::EaseOutExpo => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2f64.powf(-10.0 * t)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_fixed() {
        for e in [
            Easing::Linear,
            Easing::EaseInOutSine,
            Easing::EaseInOutCubic,
            Easing::EaseInOutQuint,
            Easing::EaseOutExpo,
        ] {
            assert!(e.apply(0.0).abs() < 1e-9, "{e:?}");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-9, "{e:?}");
        }
    }
}
