//! In-slide animations ("builds"): objects appearing, being emphasised and
//! disappearing while a slide is on screen, in a set order.
//!
//! Builds live on the [`Step`](crate::Step) of the slide they belong to. In
//! the show, each click plays the next build (plus any that run with or after
//! it); once a slide's builds are done, the next click moves on.

use serde::{Deserialize, Serialize};

/// What a build does to its object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    // Appear.
    Fade,
    Rise,
    Focus,
    Zoom,
    Pop,
    Drop,
    Fly,
    Spin,
    Typewriter,
    Words,
    CountUp,
    // Emphasis.
    Pulse,
    Wiggle,
    Glow,
    Spotlight,
    Highlight,
    Underline,
    ColorPop,
    // Disappear.
    FadeOut,
    Sink,
    ZoomOut,
    FlyOut,
    Shrink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuildKind {
    /// The object is hidden until the build plays.
    Appear,
    /// The object stays; the effect draws attention to it.
    Emphasis,
    /// The object is hidden once the build has played.
    Disappear,
}

impl Effect {
    pub const APPEAR: [Effect; 11] =
        [Effect::Fade, Effect::Rise, Effect::Focus, Effect::Zoom, Effect::Pop, Effect::Drop, Effect::Fly, Effect::Spin, Effect::Typewriter, Effect::Words, Effect::CountUp];
    pub const EMPHASIS: [Effect; 7] = [Effect::Pulse, Effect::Wiggle, Effect::Glow, Effect::Spotlight, Effect::Highlight, Effect::Underline, Effect::ColorPop];
    pub const DISAPPEAR: [Effect; 5] = [Effect::FadeOut, Effect::Sink, Effect::ZoomOut, Effect::FlyOut, Effect::Shrink];

    pub fn all() -> impl Iterator<Item = Effect> {
        Self::APPEAR.into_iter().chain(Self::EMPHASIS).chain(Self::DISAPPEAR)
    }

    pub fn kind(self) -> BuildKind {
        if Self::APPEAR.contains(&self) {
            BuildKind::Appear
        } else if Self::EMPHASIS.contains(&self) {
            BuildKind::Emphasis
        } else {
            BuildKind::Disappear
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Effect::Fade => "Fade",
            Effect::Rise => "Rise",
            Effect::Focus => "Focus",
            Effect::Zoom => "Zoom",
            Effect::Pop => "Pop",
            Effect::Drop => "Drop",
            Effect::Fly => "Fly in",
            Effect::Spin => "Spin",
            Effect::Typewriter => "Typewriter",
            Effect::Words => "Word by word",
            Effect::CountUp => "Count up",
            Effect::Pulse => "Pulse",
            Effect::Wiggle => "Wiggle",
            Effect::Glow => "Glow",
            Effect::Spotlight => "Spotlight",
            Effect::Highlight => "Highlight",
            Effect::Underline => "Underline",
            Effect::ColorPop => "Color pop",
            Effect::FadeOut => "Fade out",
            Effect::Sink => "Sink",
            Effect::ZoomOut => "Zoom out",
            Effect::FlyOut => "Fly out",
            Effect::Shrink => "Shrink",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Effect::Fade => "Fades in.",
            Effect::Rise => "Fades in while rising into place.",
            Effect::Focus => "Settles into focus from slightly larger, like a lens pulling focus.",
            Effect::Zoom => "Grows from small to full size.",
            Effect::Pop => "Pops in with a playful overshoot, like a sticker.",
            Effect::Drop => "Drops in from above and bounces.",
            Effect::Fly => "Slides in from the edge of the slide.",
            Effect::Spin => "Spins in while growing.",
            Effect::Typewriter => "Text types itself, letter by letter.",
            Effect::Words => "Text appears word by word.",
            Effect::CountUp => "Numbers in the text count up from zero.",
            Effect::Pulse => "A quick grow-and-settle.",
            Effect::Wiggle => "A short playful wiggle.",
            Effect::Glow => "A colored glow that swells and fades.",
            Effect::Spotlight => "Dims everything else on the slide.",
            Effect::Highlight => "A marker sweeps behind it.",
            Effect::Underline => "An underline sweeps in beneath it.",
            Effect::ColorPop => "Turns the theme's accent color.",
            Effect::FadeOut => "Fades away.",
            Effect::Sink => "Fades while sinking down.",
            Effect::ZoomOut => "Grows and fades toward the viewer.",
            Effect::FlyOut => "Slides out past the edge of the slide.",
            Effect::Shrink => "Shrinks away to nothing.",
        }
    }

    /// Stable id for the CLI and MCP (`rise`, `count_up`, …).
    pub fn id(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
    }

    pub fn from_id(s: &str) -> Option<Effect> {
        let want = s.trim().to_lowercase().replace([' ', '-'], "_");
        Effect::all().find(|e| e.id() == want || e.label().to_lowercase().replace(' ', "_") == want)
    }

    /// A good default duration in seconds.
    pub fn default_duration(self) -> f64 {
        match self {
            Effect::Typewriter | Effect::Words => 1.2,
            Effect::CountUp => 1.4,
            Effect::Drop | Effect::Spin => 0.8,
            Effect::Wiggle | Effect::Highlight | Effect::Underline => 0.7,
            Effect::Pop | Effect::Pulse | Effect::Shrink => 0.45,
            _ => 0.6,
        }
    }

    /// Whether the effect uses a direction.
    pub fn has_direction(self) -> bool {
        matches!(self, Effect::Fly | Effect::FlyOut)
    }
}

/// When a build starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// On the next click (or key / clicker press).
    #[default]
    OnClick,
    /// Together with the build before it.
    WithPrevious,
    /// When the build before it has finished.
    AfterPrevious,
}

impl Trigger {
    pub const ALL: [Trigger; 3] = [Trigger::OnClick, Trigger::WithPrevious, Trigger::AfterPrevious];

    pub fn label(self) -> &'static str {
        match self {
            Trigger::OnClick => "On click",
            Trigger::WithPrevious => "With previous",
            Trigger::AfterPrevious => "After previous",
        }
    }
}

/// Which side a flying object comes from (or leaves toward).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub const ALL: [Direction; 4] = [Direction::Left, Direction::Right, Direction::Up, Direction::Down];

    pub fn label(self) -> &'static str {
        match self {
            Direction::Left => "Left",
            Direction::Right => "Right",
            Direction::Up => "Top",
            Direction::Down => "Bottom",
        }
    }

    /// Unit vector toward this side (y down).
    pub fn vector(self) -> (f64, f64) {
        match self {
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
            Direction::Up => (0.0, -1.0),
            Direction::Down => (0.0, 1.0),
        }
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn is_default_direction(d: &Direction) -> bool {
    *d == Direction::default()
}

fn is_on_click(t: &Trigger) -> bool {
    *t == Trigger::OnClick
}

/// One in-slide animation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Build {
    /// The animated element.
    pub element: String,
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "is_on_click")]
    pub trigger: Trigger,
    /// Seconds.
    pub duration: f64,
    /// Seconds after the trigger.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub delay: f64,
    #[serde(default, skip_serializing_if = "is_default_direction")]
    pub direction: Direction,
}

impl Build {
    pub fn new(element: impl Into<String>, effect: Effect) -> Self {
        Build { element: element.into(), effect, trigger: Trigger::OnClick, duration: effect.default_duration(), delay: 0.0, direction: Direction::default() }
    }

    pub fn with_trigger(mut self, trigger: Trigger) -> Self {
        self.trigger = trigger;
        self
    }

    pub fn with_delay(mut self, delay: f64) -> Self {
        self.delay = delay;
        self
    }
}

/// Click groups of a step's builds: each group starts with an on-click build
/// (or the builds that run on arrival) and includes the builds that follow it
/// automatically. Returns index ranges into `builds`.
pub fn click_groups(builds: &[Build]) -> Vec<std::ops::Range<usize>> {
    let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
    for (i, b) in builds.iter().enumerate() {
        match groups.last_mut() {
            Some(g) if b.trigger != Trigger::OnClick => g.end = i + 1,
            _ => groups.push(i..i + 1),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_have_kinds_ids_and_round_trip() {
        assert_eq!(Effect::all().count(), 23);
        for e in Effect::all() {
            assert_eq!(Effect::from_id(&e.id()), Some(e), "{e:?}");
            assert!(e.default_duration() > 0.0);
        }
        assert_eq!(Effect::CountUp.id(), "count_up");
        assert_eq!(Effect::from_id("Fly in"), Some(Effect::Fly));
        assert_eq!(Effect::Spotlight.kind(), BuildKind::Emphasis);
        assert_eq!(Effect::Shrink.kind(), BuildKind::Disappear);
        let b = Build::new("title", Effect::Rise);
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(json, r#"{"element":"title","effect":"rise","duration":0.6}"#);
        assert_eq!(serde_json::from_str::<Build>(&json).unwrap(), b);
    }

    #[test]
    fn groups_start_at_each_click() {
        let b = |t| Build::new("x", Effect::Fade).with_trigger(t);
        let builds = [b(Trigger::AfterPrevious), b(Trigger::WithPrevious), b(Trigger::OnClick), b(Trigger::AfterPrevious), b(Trigger::OnClick)];
        assert_eq!(click_groups(&builds), vec![0..2, 2..4, 4..5]);
        assert!(click_groups(&[]).is_empty());
    }
}
