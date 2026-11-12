//! Records footage for the marketing video: the real app, driven through
//! egui_kittest on a real GPU device, piped frame by frame into FFmpeg.
//!
//! `GLIDEDECK_VIDEO_OUT=<dir> cargo test -p glidedeck video_footage -- --ignored --nocapture`
//!
//! Each clip is written as `<dir>/<name>.mp4` (30 frames a second) with a
//! `<name>.json` beside it holding named moments and the pointer position for
//! every frame, so the video can cut on them and draw its own cursor.
//! `GLIDEDECK_VIDEO_CLIPS=a,b` records only those clips. Needs FFmpeg on the PATH.
//! The video itself lives in `marketing/video`.

use crate::app::GlideDeckApp;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use glide_core::{Document, Step, TransitionStyle};
use std::io::Write;
use std::path::{Path, PathBuf};

type H = Harness<'static, GlideDeckApp>;

/// Simulated steps per second; every second step becomes a video frame.
const STEPS: f32 = 60.0;

fn build(px: (u32, u32), ppp: f32, dark: bool, doc: Option<Document>) -> H {
    let mut h = Harness::builder()
        .with_size(egui::vec2(px.0 as f32 / ppp, px.1 as f32 / ppp))
        .with_pixels_per_point(ppp)
        .with_step_dt(1.0 / STEPS)
        .wgpu()
        .build_eframe(move |cc| {
            let mut app = GlideDeckApp::new(cc, None).expect("app starts");
            if let Some(doc) = doc {
                app.doc = doc;
                app.history.reset(&app.doc);
                app.current_step = Some(0);
                app.pending_fit = true;
            }
            app
        });
    h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
    for _ in 0..30 {
        h.step();
    }
    h
}

struct Recorder {
    ffmpeg: std::process::Child,
    json: PathBuf,
    ppp: f32,
    /// Pointer position in physical pixels, and whether the button is down, per frame.
    track: Vec<(f32, f32, bool)>,
    /// Named moments, as frame numbers, for cutting the video.
    marks: Vec<(String, usize)>,
    pos: Pos2,
    down: bool,
    started: std::time::Instant,
}

impl Recorder {
    fn new(dir: &Path, name: &str, px: (u32, u32), ppp: f32) -> Self {
        std::fs::create_dir_all(dir).unwrap();
        let ffmpeg = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{}x{}", px.0, px.1), "-r", "30", "-i", "-"])
            .args(["-c:v", "libx264", "-preset", "medium", "-crf", "12", "-pix_fmt", "yuv420p", "-g", "15", "-movflags", "+faststart"])
            .arg(dir.join(format!("{name}.mp4")))
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("ffmpeg on the PATH");
        Recorder {
            ffmpeg,
            json: dir.join(format!("{name}.json")),
            ppp,
            track: Vec::new(),
            marks: Vec::new(),
            pos: Pos2::new(-100.0, -100.0),
            down: false,
            started: std::time::Instant::now(),
        }
    }

    fn capture(&mut self, h: &mut H) {
        let img = h.render().expect("render");
        self.ffmpeg.stdin.as_mut().unwrap().write_all(img.as_raw()).expect("ffmpeg takes the frame");
        self.track.push((self.pos.x * self.ppp, self.pos.y * self.ppp, self.down));
    }

    fn mark(&mut self, name: &str) {
        self.marks.push((name.to_owned(), self.track.len()));
    }

    /// Lets the app run for `seconds`, recording.
    fn roll(&mut self, h: &mut H, seconds: f32) {
        for _ in 0..(seconds * 30.0).round() as usize {
            h.step();
            h.step();
            self.capture(h);
        }
    }

    /// Glides the pointer to `to` over `seconds`, easing in and out.
    fn move_to(&mut self, h: &mut H, to: Pos2, seconds: f32) {
        let from = self.pos;
        let n = ((seconds * STEPS).round() as usize).max(2);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let e = t * t * (3.0 - 2.0 * t);
            self.pos = from + (to - from) * e;
            h.event(Event::PointerMoved(self.pos));
            h.step();
            if i % 2 == 0 {
                self.capture(h);
            }
        }
    }

    fn press(&mut self, h: &mut H, pressed: bool) {
        h.event(Event::PointerButton { pos: self.pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        self.down = pressed;
    }

    /// Moves to `at` and clicks there.
    fn click(&mut self, h: &mut H, at: Pos2, travel: f32) {
        self.move_to(h, at, travel);
        self.roll(h, 0.1);
        self.press(h, true);
        self.roll(h, 0.1);
        self.press(h, false);
        self.roll(h, 0.1);
    }

    /// Moves to the widget with this accessibility label and clicks it.
    fn click_label(&mut self, h: &mut H, label: &str, travel: f32) {
        let at = h.get_by_label(label).rect().center();
        self.click(h, at, travel);
    }

    fn click_label_contains(&mut self, h: &mut H, label: &str, travel: f32) {
        let at = h.get_by_label_contains(label).rect().center();
        self.click(h, at, travel);
    }

    fn type_text(&mut self, h: &mut H, text: &str, per_key: f32) {
        for ch in text.chars() {
            h.event(Event::Text(ch.to_string()));
            self.roll(h, per_key);
        }
    }

    fn finish(mut self) {
        drop(self.ffmpeg.stdin.take());
        assert!(self.ffmpeg.wait().expect("ffmpeg runs").success());
        let frames: Vec<String> = self.track.iter().map(|(x, y, d)| format!("[{x:.1},{y:.1},{}]", u8::from(*d))).collect();
        let marks: Vec<String> = self.marks.iter().map(|(name, at)| format!("\"{name}\":{at}")).collect();
        let json = format!("{{\"fps\":30,\"frames\":{},\"marks\":{{{}}},\"cursor\":[{}]}}", self.track.len(), marks.join(","), frames.join(","));
        std::fs::write(&self.json, json).unwrap();
        eprintln!("{}: {} frames in {:.0?}", self.json.display(), self.track.len(), self.started.elapsed());
    }
}

const HD: (u32, u32) = (1920, 1080);
/// A phone screen, for the vertical cut.
const TALL: (u32, u32) = (1080, 1920);
/// The editor is recorded larger than the video, so the edit can punch in on it.
const EDITOR: (u32, u32) = (2560, 1440);
const EDITOR_PPP: f32 = 2560.0 / 1440.0;

/// The show, started with its controls already faded out.
fn show(px: (u32, u32), doc: Option<Document>, step: usize) -> H {
    let mut h = build(px, 1.0, true, doc);
    let ctx = h.ctx.clone();
    h.state_mut().start_presenting(&ctx, step, Some(false));
    for _ in 0..300 {
        h.step();
    }
    h
}

/// The welcome canvas with its path recut for the video: the promise, the
/// big picture, one detail, the portal all the way down, and back out.
fn hero_document() -> Document {
    let mut doc = crate::demo::demo_document();
    let old = std::mem::take(&mut doc.path);
    let stop = |frame: &str, style: TransitionStyle, duration: f64| {
        let mut step = old.iter().find(|s| s.frame == frame).cloned().unwrap_or_else(|| Step::new(frame));
        step.transition.style = style;
        step.transition.duration = duration;
        step
    };
    let overview = |style, duration| {
        let mut step = Step::new("frame-overview");
        step.transition.style = style;
        step.transition.duration = duration;
        step
    };
    doc.path = vec![
        overview(TransitionStyle::Smooth, 1.4),
        stop("frame-title", TransitionStyle::Smooth, 1.4),
        overview(TransitionStyle::Smooth, 1.3),
        stop("frame-canvas", TransitionStyle::Swoop, 1.4),
        stop("frame-portal", TransitionStyle::Smooth, 1.4),
        stop("frame-depth-4", TransitionStyle::Direct, 3.4),
        overview(TransitionStyle::Direct, 2.8),
    ];
    doc
}

/// A quiet spot on the canvas for the pointer to wait.
fn canvas_rest(h: &H) -> Pos2 {
    let r = h.state().canvas_rect;
    Pos2::new(r.left() + r.width() * 0.62, r.top() + r.height() * 0.7)
}

#[test]
#[ignore = "records footage for the marketing video; run explicitly"]
fn video_footage() {
    let Some(dir) = std::env::var_os("GLIDEDECK_VIDEO_OUT") else { return };
    let dir = PathBuf::from(dir);
    let only = std::env::var("GLIDEDECK_VIDEO_CLIPS").ok();
    let wanted = |name: &str| only.as_deref().is_none_or(|o| o.split(',').any(|c| c.trim() == name));
    let pptx = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../marketing/video/assets/launch-plan.pptx");

    for (suffix, px) in [("", HD), ("-tall", TALL)] {
        // The welcome show, recut: the promise, the big picture, a detail, a million times deeper, and back.
        let name = format!("hero{suffix}");
        if wanted(&name) {
            let mut h = show(px, Some(hero_document()), 0);
            let mut rec = Recorder::new(&dir, &name, px, 1.0);
            rec.mark("start");
            rec.roll(&mut h, 2.0);
            for (mark, hold) in [("title", 3.6), ("highlight", 1.6), ("overview", 3.0), ("detail", 3.0), ("portal", 2.2), ("deep", 4.2), ("back", 4.4)] {
                rec.mark(mark);
                h.key_press(Key::ArrowRight);
                rec.roll(&mut h, hold);
            }
            rec.mark("end");
            rec.finish();
        }

        // The Carousel template: slides glide past over drifting parallax layers.
        let name = format!("carousel{suffix}");
        if wanted(&name) {
            let mut h = show(px, Some(crate::design::build_template(crate::design::Template::Carousel)), 1);
            let mut rec = Recorder::new(&dir, &name, px, 1.0);
            rec.roll(&mut h, 0.6);
            for i in 0..4 {
                rec.mark(&format!("glide{i}"));
                h.key_press(Key::ArrowRight);
                rec.roll(&mut h, 2.2);
            }
            rec.finish();
        }
    }

    // A PowerPoint deck arrives as a grid; Arrange reshapes it three ways, then the show plays.
    if wanted("arrange") {
        let mut h = build(EDITOR, EDITOR_PPP, true, Some(Document::new("Launch plan")));
        let mut rec = Recorder::new(&dir, "arrange", EDITOR, EDITOR_PPP);
        rec.pos = Pos2::new(900.0, 520.0);
        rec.roll(&mut h, 0.6);
        rec.mark("import");
        let ctx = h.ctx.clone();
        h.state_mut().import_pptx(&ctx, &pptx);
        rec.roll(&mut h, 2.2);
        for kind in ["Spiral", "Orbits", "Carousel"] {
            rec.mark(&format!("open-{kind}"));
            rec.click_label(&mut h, "Arrange", 0.7);
            rec.roll(&mut h, 0.4);
            rec.click_label(&mut h, &format!("Arrange as {kind}"), 0.5);
            rec.roll(&mut h, 0.5);
            rec.click_label_contains(&mut h, "Arrange 9 slides", 0.5);
            rec.mark(&format!("applied-{kind}"));
            let rest = canvas_rest(&h);
            rec.move_to(&mut h, rest, 0.6);
            rec.roll(&mut h, 2.4);
        }
        rec.mark("present");
        // The pointer leaves the window, so the show does not draw it.
        h.event(Event::PointerGone);
        let ctx = h.ctx.clone();
        h.state_mut().start_presenting(&ctx, 0, Some(false));
        rec.pos = Pos2::new(-100.0, -100.0);
        rec.roll(&mut h, 1.2);
        for i in 0..3 {
            rec.mark(&format!("show{i}"));
            h.key_press(Key::ArrowRight);
            rec.roll(&mut h, 2.0);
        }
        rec.finish();
    }

    // One click animates a slide; the Share menu lists every export.
    if wanted("animate") {
        let mut h = build(EDITOR, EDITOR_PPP, true, Some(Document::new("Launch plan")));
        let ctx = h.ctx.clone();
        h.state_mut().import_pptx(&ctx, &pptx);
        // A saved deck, so AutoSave has somewhere to write and every export is offered.
        let saved = std::env::temp_dir().join("glidedeck-video");
        std::fs::create_dir_all(&saved).unwrap();
        h.state_mut().path = Some(saved.join("launch-plan.glide.json"));
        for _ in 0..60 {
            h.step();
        }
        h.state_mut().go_to_step(3);
        for _ in 0..150 {
            h.step();
        }
        let mut rec = Recorder::new(&dir, "animate", EDITOR, EDITOR_PPP);
        rec.pos = canvas_rest(&h);
        rec.roll(&mut h, 0.8);
        rec.mark("tab");
        rec.click_label(&mut h, "Animate", 0.7);
        rec.roll(&mut h, 0.5);
        rec.mark("auto");
        rec.click_label(&mut h, "Auto-animate", 0.6);
        for _ in 0..(5 * 30) {
            rec.roll(&mut h, 1.0 / 30.0);
            if h.state().anim_preview.is_none() {
                break;
            }
        }
        rec.roll(&mut h, 0.8);
        rec.mark("share");
        rec.click_label_contains(&mut h, "Share", 0.8);
        rec.roll(&mut h, 0.5);
        for item in ["Web page (HTML)", "PDF", "PowerPoint (Morph)"] {
            rec.mark(item);
            let at = h.get_by_label_contains(item).rect().center();
            rec.move_to(&mut h, at, 0.45);
            rec.roll(&mut h, 0.8);
        }
        rec.roll(&mut h, 0.6);
        rec.finish();
    }

    // The path strip: click a stop and the editor flies there. Then the command search.
    if wanted("path") {
        let mut h = build(EDITOR, EDITOR_PPP, true, None);
        let mut rec = Recorder::new(&dir, "path", EDITOR, EDITOR_PPP);
        rec.pos = canvas_rest(&h);
        rec.roll(&mut h, 0.8);
        for stop in ["Slide 3: Infinite canvas", "Slide 5: Motion built in", "Slide 2: Presentations that move", "Slide 1: Overview"] {
            rec.mark(stop);
            rec.click_label_contains(&mut h, stop, 0.6);
            rec.roll(&mut h, 1.8);
        }
        rec.mark("search");
        h.key_press_modifiers(Modifiers::COMMAND, Key::K);
        rec.roll(&mut h, 0.5);
        rec.type_text(&mut h, "light theme", 0.1);
        rec.roll(&mut h, 0.6);
        h.key_press(Key::Enter);
        rec.roll(&mut h, 2.0);
        rec.finish();
    }
}
