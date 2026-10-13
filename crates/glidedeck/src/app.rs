use crate::canvas::{Drag, Navigation};
use crate::design::{AlignOp, LayoutKind, Template};
use crate::galleries::{DesignCheckState, IconPicker, Previews};
use crate::history::History;
use crate::present::Presenting;
use crate::slides::Thumbnails;
use crate::theme::{self, icon};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, ThemePreference};
use eframe::wgpu;
use glam::DVec2;
use std::path::{Path, PathBuf};
use glide_core::{
    assets, Aabb, Camera, CameraTween, Color, Document, Easing, Element, ElementKind, Frame, IconStyle, Shadow, Step, TextRole,
    Theme, Transform,
};
use glide_render::{CanvasRenderer, RenderStats, RenderTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Hand,
    Frame,
    Rect,
    Ellipse,
    Text,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Hand => "Hand",
            Tool::Frame => "Frame",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Text => "Text",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => icon::CURSOR,
            Tool::Hand => icon::HAND,
            Tool::Frame => icon::FRAME_CORNERS,
            Tool::Rect => icon::SQUARE,
            Tool::Ellipse => icon::CIRCLE,
            Tool::Text => icon::TEXT_T,
        }
    }

    pub fn key(self) -> Key {
        match self {
            Tool::Select => Key::V,
            Tool::Hand => Key::H,
            Tool::Frame => Key::F,
            Tool::Rect => Key::R,
            Tool::Ellipse => Key::O,
            Tool::Text => Key::T,
        }
    }

    pub const ALL: [Tool; 6] = [Tool::Select, Tool::Hand, Tool::Frame, Tool::Rect, Tool::Ellipse, Tool::Text];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonTab {
    Home,
    Insert,
    Design,
    Transitions,
    Present,
    View,
}

/// Everything the user can trigger from the ribbon, menus, panels or keyboard.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    ExportPptx,
    ExportPng,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    Duplicate,
    SelectAll,
    Deselect,
    BringToFront,
    SendToBack,
    SetTool(Tool),
    InsertImage,
    InsertVideo,
    FrameFromView,
    AddSelectedFrameToPath,
    GoToStep(usize),
    MoveStep { from: usize, to: usize },
    DuplicateStep(usize),
    RemoveStep(usize),
    ImportPptx,
    ExportPdf,
    OpenRecent(PathBuf),
    ClearRecent,
    /// Formats the text being edited, or the selected text boxes.
    FormatText(crate::textformat::TextFormat),
    /// Starts the show; presenter view is used automatically with a second display.
    Present { from_start: bool },
    /// Starts the show with presenter view, even on a single display.
    PresenterView { from_start: bool },
    FitAll,
    FitSelection,
    ResetView,
    ZoomIn,
    ZoomOut,
    RotateView(f64),
    Nudge(DVec2),
    ToggleGrid,
    ToggleFrameOutlines,
    ToggleSlides,
    ToggleInspector,
    SetTheme(ThemePreference),
    ToggleShortcuts,
    // Design features.
    ShowIcons,
    ShowTemplates,
    ShowLayouts,
    ToggleDesignCheck,
    InsertLayout(LayoutKind),
    NewFromTemplate(Template),
    ApplyTheme(usize),
    Align(AlignOp),
    InsertText(TextRole),
    ApplyTextRole(TextRole),
    SetShadow(Option<Shadow>),
    ToggleGradient,
    /// Edit the selected text box in place.
    EditText,
}

const fn sc(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(modifiers, key)
}

pub const SC_NEW: KeyboardShortcut = sc(Modifiers::COMMAND, Key::N);
pub const SC_OPEN: KeyboardShortcut = sc(Modifiers::COMMAND, Key::O);
pub const SC_SAVE: KeyboardShortcut = sc(Modifiers::COMMAND, Key::S);
pub const SC_SAVE_AS: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
pub const SC_UNDO: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Z);
pub const SC_REDO: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
pub const SC_REDO_ALT: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Y);
pub const SC_DUPLICATE: KeyboardShortcut = sc(Modifiers::COMMAND, Key::D);
pub const SC_SELECT_ALL: KeyboardShortcut = sc(Modifiers::COMMAND, Key::A);
pub const SC_FRONT: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::CloseBracket);
pub const SC_BACK: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::OpenBracket);
pub const SC_RESET_VIEW: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Num0);
pub const SC_FIT_ALL: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Num1);
pub const SC_FIT_SELECTION: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Num2);
pub const SC_ZOOM_IN: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Equals);
pub const SC_ZOOM_OUT: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Minus);
pub const SC_FRAME_VIEW: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::F);
pub const SC_PRESENT: KeyboardShortcut = sc(Modifiers::NONE, Key::F5);
pub const SC_PRESENT_HERE: KeyboardShortcut = sc(Modifiers::SHIFT, Key::F5);
/// A show to start as soon as the window opens.
#[derive(Clone, Copy, Debug)]
pub struct StartShow {
    pub step: usize,
    /// `Some(true)` forces presenter view; `None` decides by the number of displays.
    pub presenter: Option<bool>,
}

pub const SC_PRESENTER: KeyboardShortcut = sc(Modifiers::ALT, Key::F5);
pub const SC_BOLD: KeyboardShortcut = sc(Modifiers::COMMAND, Key::B);
pub const SC_ITALIC: KeyboardShortcut = sc(Modifiers::COMMAND, Key::I);
pub const SC_UNDERLINE: KeyboardShortcut = sc(Modifiers::COMMAND, Key::U);
pub const SC_BIGGER: KeyboardShortcut = sc(Modifiers::COMMAND, Key::CloseBracket);
pub const SC_SMALLER: KeyboardShortcut = sc(Modifiers::COMMAND, Key::OpenBracket);
pub const SC_CLEAR_FORMAT: KeyboardShortcut = sc(Modifiers::COMMAND, Key::Backslash);
pub const SC_INSERT_IMAGE: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::I);
pub const SC_CUT: KeyboardShortcut = sc(Modifiers::COMMAND, Key::X);
pub const SC_COPY: KeyboardShortcut = sc(Modifiers::COMMAND, Key::C);
pub const SC_PASTE: KeyboardShortcut = sc(Modifiers::COMMAND, Key::V);
pub const SC_ICONS: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::K);
pub const SC_LAYOUTS: KeyboardShortcut = sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::L);

pub struct GlideDeckApp {
    pub doc: Document,
    pub path: Option<PathBuf>,
    saved: Document,
    pub history: History,
    pub camera: Camera,
    pub nav: Navigation,
    pub selection: Vec<String>,
    pub tool: Tool,
    pub drag: Drag,
    pub renderer: CanvasRenderer,
    pub target: Option<RenderTarget>,
    pub texture_id: Option<egui::TextureId>,
    /// Extra off-screen views (e.g. the presenter's next-slide preview).
    snapshots: std::collections::HashMap<&'static str, (RenderTarget, egui::TextureId)>,
    pub show_grid: bool,
    /// Outline every frame (otherwise frames only show on hover/selection).
    pub show_frames: bool,
    /// Trackpad-friendly navigation: scroll pans, Ctrl/Cmd+scroll zooms.
    pub scroll_pans: bool,
    pub show_slides: bool,
    pub show_inspector: bool,
    pub ribbon_tab: RibbonTab,
    pub stats: RenderStats,
    pub canvas_rect: egui::Rect,
    pub cursor_world: Option<DVec2>,
    pub hovered: Option<String>,
    pub focus_text_editor: bool,
    pub pending_fit: bool,
    /// Start presenting on the first frame (`glidedeck present`).
    pub start_show: Option<StartShow>,
    /// A PowerPoint file given on the command line, imported on the first frame.
    import_on_start: Option<PathBuf>,
    /// File → Open recent.
    pub recent: crate::recent::RecentFiles,
    /// The slide (path step) the editor is on.
    pub current_step: Option<usize>,
    pub thumbs: Thumbnails,
    pub presenting: Option<Presenting>,
    /// Clickable frame name tags drawn on the canvas last frame.
    pub frame_tags: Vec<(egui::Rect, String)>,
    /// Asset folder for documents that have not been saved yet.
    session_dir: PathBuf,
    /// A paste event already handled the current Ctrl+V press.
    pub paste_consumed: bool,
    pub show_shortcuts: bool,
    pub icon_picker: IconPicker,
    pub show_templates: bool,
    pub show_layouts: bool,
    pub design_check: DesignCheckState,
    pub previews: Previews,
    /// Smart-guide lines shown while dragging: (vertical?, screen coordinate).
    pub guides: Vec<(bool, f32)>,
    /// Play/pause buttons drawn over videos last frame: (rect, element id).
    pub video_buttons: Vec<(egui::Rect, String)>,
    /// In-place text editing on the canvas.
    pub text_edit: Option<crate::textedit::TextEditSession>,
    toast: Option<(String, f64)>,
    title_shown: String,
    allow_close: bool,
}

impl GlideDeckApp {
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>) -> anyhow::Result<Self> {
        theme::apply(&cc.egui_ctx);
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("the wgpu renderer is not available"))?;
        let renderer = CanvasRenderer::new(&rs.device, &rs.queue);

        let mut recent = crate::recent::RecentFiles::load();
        let is_pptx = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pptx"));
        let import_on_start = file.clone().filter(|p| is_pptx(p) && p.exists());
        let file = file.filter(|p| !is_pptx(p));
        if let Some(p) = file.as_ref().filter(|p| p.exists()) {
            recent.add(p);
        }
        let (doc, path) = match file {
            Some(p) if p.exists() => (Document::load(&p)?, Some(p)),
            Some(p) => (Document::new(title_from_path(&p)), Some(p)),
            None => (crate::demo::demo_document(), None),
        };
        let session_dir = std::env::temp_dir().join("glidedeck-unsaved").join(format!("session-{}", std::process::id()));
        let mut app = GlideDeckApp {
            history: History::new(&doc),
            saved: doc.clone(),
            current_step: (!doc.path.is_empty()).then_some(0),
            doc,
            path,
            camera: Camera::default(),
            nav: Navigation::default(),
            selection: Vec::new(),
            tool: Tool::Select,
            drag: Drag::None,
            renderer,
            target: None,
            texture_id: None,
            snapshots: Default::default(),
            show_grid: true,
            show_frames: false,
            scroll_pans: false,
            show_slides: true,
            show_inspector: true,
            ribbon_tab: RibbonTab::Home,
            stats: RenderStats::default(),
            canvas_rect: egui::Rect::NOTHING,
            cursor_world: None,
            hovered: None,
            focus_text_editor: false,
            pending_fit: true,
            start_show: None,
            import_on_start,
            recent,
            thumbs: Thumbnails::default(),
            presenting: None,
            frame_tags: Vec::new(),
            session_dir,
            paste_consumed: false,
            show_shortcuts: false,
            icon_picker: IconPicker::default(),
            show_templates: false,
            show_layouts: false,
            design_check: DesignCheckState::default(),
            previews: Previews::default(),
            guides: Vec::new(),
            video_buttons: Vec::new(),
            text_edit: None,
            toast: None,
            title_shown: String::new(),
            allow_close: false,
        };
        app.sync_base_dir();
        Ok(app)
    }

    /// The document's theme (or the default "Paper" theme).
    pub fn theme(&self) -> Theme {
        self.doc.theme.clone().unwrap_or_else(glide_core::theme::default_theme)
    }

    /// Transform for new content at the view center, sized in screen units.
    fn view_placement(&self) -> Transform {
        Transform {
            x: self.camera.center.x,
            y: self.camera.center.y,
            scale: 1.0 / self.camera.zoom,
            rotation: self.camera.rotation.to_degrees(),
        }
    }

    pub fn insert_icon(&mut self, name: &str, style: IconStyle, color: Color) {
        let mut el = Element::new(ElementKind::Icon(glide_core::Icon { name: name.into(), style, size: 96.0, color }))
            .with_transform(self.view_placement());
        el.name = name.replace('-', " ");
        // Stagger repeated inserts so they don't stack exactly.
        let n = self.selection.len().min(8) as f64;
        let off = self.camera.screen_delta_to_world(DVec2::splat(28.0 * n));
        el.transform.set_position(el.transform.position() + off);
        let id = self.doc.add(el);
        self.selection = vec![id];
        self.tool = Tool::Select;
    }

    pub fn insert_text_role(&mut self, role: TextRole) {
        let theme = self.theme();
        let el = Element::new(ElementKind::Text(crate::design::styled_text(&theme, role))).with_transform(self.view_placement());
        let id = self.doc.add(el);
        self.selection = vec![id];
        self.focus_text_editor = true;
    }

    /// Adds a pre-designed slide next to the current one (or in view) and flies there.
    pub fn insert_layout(&mut self, kind: LayoutKind) {
        let theme = self.theme();
        let anchor = self.current_step.and_then(|i| self.doc.step_frame(i)).cloned();
        let (center, width, rotation) = match &anchor {
            Some(f) => crate::layouts::slot_after(&self.doc, f),
            None => {
                let vp = self.viewport();
                let w = vp.x.min(vp.y * 16.0 / 9.0) * 0.86 / self.camera.zoom;
                let rotation = self.camera.rotation.to_degrees();
                (crate::layouts::free_spot(&self.doc, self.camera.center, w, rotation), w, rotation)
            }
        };
        let id = crate::design::place_layout(&mut self.doc, kind, &theme, center, width, rotation);
        self.insert_step(&id);
        if let Some(i) = self.current_step {
            self.go_to_step(i);
        }
        self.show_layouts = false;
    }

    pub fn new_from_template(&mut self, t: Template) {
        if self.confirm_discard() {
            let doc = crate::design::build_template(t);
            self.replace_document(doc, None);
            self.show_templates = false;
        }
    }

    /// Folder that relative media references resolve against.
    pub fn base_dir(&self) -> PathBuf {
        match self.path.as_ref().and_then(|p| p.parent()) {
            Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
            Some(_) => PathBuf::from("."),
            None => self.session_dir.clone(),
        }
    }

    fn sync_base_dir(&mut self) {
        let dir = self.base_dir();
        self.renderer.set_base_dir(Some(dir));
    }

    pub fn is_dirty(&self) -> bool {
        self.doc != self.saved
    }

    pub fn viewport(&self) -> DVec2 {
        DVec2::new(self.canvas_rect.width() as f64, self.canvas_rect.height() as f64).max(DVec2::ONE)
    }

    pub fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.toast = Some((msg.into(), ctx.input(|i| i.time) + 3.5));
    }

    pub fn toast(&self) -> Option<&(String, f64)> {
        self.toast.as_ref()
    }

    // --- camera helpers ------------------------------------------------------

    pub fn animate_to(&mut self, target: Camera) {
        let span = (target.zoom / self.camera.zoom).ln().abs();
        let duration = (0.55 + 0.12 * span).min(1.8);
        self.nav.tween = Some(CameraTween::new(self.camera, target, duration, Easing::EaseInOutCubic));
        self.nav.clear_pending();
    }

    pub fn world_bounds(&mut self, ids: Option<&[String]>) -> Aabb {
        let mut b = Aabb::EMPTY;
        for el in &self.doc.elements {
            if el.hidden || ids.is_some_and(|ids| !ids.contains(&el.id)) {
                continue;
            }
            b = b.union(el.world_aabb(glide_render::local_size(&mut self.renderer.text, &el.kind)));
        }
        b
    }

    pub fn fit_all(&mut self, animate: bool) {
        let bounds = self.world_bounds(None);
        let target = if bounds.is_empty() {
            Camera::default()
        } else {
            Camera::framing(bounds, 0.0, self.viewport(), 56.0)
        };
        if animate { self.animate_to(target) } else { self.camera = target }
    }

    /// Frames a single element, aligning the view with its rotation.
    pub fn frame_element(&mut self, id: &str) {
        let Some(el) = self.doc.get(id).cloned() else { return };
        let size = (self.renderer.local_size(&el) * el.transform.scale.abs()).max(DVec2::splat(1e-12));
        let margin = if el.kind.is_frame() { 28.0 } else { 64.0 };
        let avail = (self.viewport() - DVec2::splat(2.0 * margin)).max(DVec2::ONE);
        let zoom = (avail.x / size.x).min(avail.y / size.y);
        self.animate_to(Camera {
            center: el.transform.position(),
            zoom,
            rotation: el.transform.rotation.to_radians(),
        });
    }

    pub fn fit_selection(&mut self) {
        match self.selection.as_slice() {
            [] => {}
            [one] => {
                let id = one.clone();
                self.frame_element(&id);
            }
            many => {
                let ids = many.to_vec();
                let bounds = self.world_bounds(Some(&ids));
                let target = Camera::framing(bounds, self.camera.rotation, self.viewport(), 64.0);
                self.animate_to(target);
            }
        }
    }

    /// Travels to a step along its transition, as in the presentation.
    pub fn go_to_step(&mut self, index: usize) {
        let Some(frame) = self.doc.step_frame(index).cloned() else { return };
        self.current_step = Some(index);
        self.selection = vec![frame.id.clone()];
        let Some(target) = Camera::for_frame(&frame, self.viewport(), 28.0) else { return };
        let t = self.doc.path[index].transition;
        let duration = t.duration.min(1.2);
        self.nav.tween =
            Some(CameraTween::new(self.camera, target, duration, t.easing).with_style(t.style, self.viewport()));
        self.nav.clear_pending();
    }

    /// Steps whose frame is `id`.
    pub fn steps_of(&self, id: &str) -> Vec<usize> {
        self.doc.path.iter().enumerate().filter(|(_, s)| s.frame == id).map(|(i, _)| i).collect()
    }

    /// Inserts a step for `frame_id` after the current step and makes it current.
    pub fn insert_step(&mut self, frame_id: &str) {
        let at = self.current_step.map(|i| i + 1).unwrap_or(self.doc.path.len()).min(self.doc.path.len());
        self.doc.path.insert(at, Step::new(frame_id));
        self.current_step = Some(at);
    }

    /// A new invisible 16:9 frame matching the current view.
    pub fn frame_from_view(&mut self) -> String {
        let vp = self.viewport();
        let w_screen = vp.x.min(vp.y * 16.0 / 9.0) * 0.86;
        let mut el = Element::new(ElementKind::Frame(Frame::widescreen(w_screen)));
        el.transform = Transform {
            x: self.camera.center.x,
            y: self.camera.center.y,
            scale: 1.0 / self.camera.zoom,
            rotation: self.camera.rotation.to_degrees(),
        };
        el.name = format!("Frame {}", self.doc.elements.iter().filter(|e| e.kind.is_frame()).count() + 1);
        let id = self.doc.add(el);
        self.insert_step(&id);
        self.selection = vec![id.clone()];
        id
    }

    /// Toggles playback of a video element. Returns the new playing state.
    pub fn toggle_video(&mut self, id: &str) -> bool {
        let Some(glide_core::Element { kind: ElementKind::Video(m), .. }) = self.doc.get(id).cloned() else { return false };
        let playing = matches!(self.renderer.video_status(&m.path), glide_render::VideoStatus::Ready { playing: true, .. });
        self.renderer.video_set_playing(&m.path, !playing, m.looping, m.muted)
    }

    /// Wraps the selection in a new invisible 16:9 frame (aligned with the
    /// view rotation) and adds it to the path after the current slide.
    pub fn frame_around_selection(&mut self) {
        let ids: Vec<String> = self.selection.iter().filter(|id| self.doc.get(id).is_some_and(|e| !e.kind.is_frame())).cloned().collect();
        if ids.is_empty() {
            return;
        }
        // Bounds in view-aligned space so rotated views get snug frames.
        let rot = self.camera.rotation;
        let to_view = glam::DVec2::from_angle(-rot);
        let mut b = Aabb::EMPTY;
        for id in &ids {
            let Some(el) = self.doc.get(id).cloned() else { continue };
            let size = self.renderer.local_size(&el);
            for c in el.world_corners(size) {
                b = b.including(to_view.rotate(c));
            }
        }
        if b.is_empty() {
            return;
        }
        let pad = b.size().max_element() * 0.08;
        let size = b.size() + DVec2::splat(2.0 * pad);
        let w = size.x.max(size.y * 16.0 / 9.0);
        let center = glam::DVec2::from_angle(rot).rotate(b.center());
        let mut el = Element::new(ElementKind::Frame(Frame::widescreen(w)));
        el.transform = Transform { x: center.x, y: center.y, scale: 1.0, rotation: rot.to_degrees() };
        el.name = format!("Frame {}", self.doc.elements.iter().filter(|e| e.kind.is_frame()).count() + 1);
        // Frames sit beneath what they group.
        let id = self.doc.add(el);
        if let Some(i) = self.doc.index_of(&id) {
            let f = self.doc.elements.remove(i);
            self.doc.elements.insert(0, f);
        }
        self.insert_step(&id);
        self.selection = vec![id];
    }

    // --- actions --------------------------------------------------------------

    pub fn perform(&mut self, ctx: &egui::Context, action: Action) {
        let viewport = self.viewport();
        match action {
            Action::New => {
                if self.confirm_discard() {
                    self.replace_document(Document::new("Untitled"), None);
                }
            }
            Action::Open => {
                if self.confirm_discard()
                    && let Some(p) = rfd::FileDialog::new()
                        .add_filter("GlideDeck or PowerPoint", &["json", "pptx"])
                        .add_filter("GlideDeck document", &["json"])
                        .add_filter("PowerPoint presentation", &["pptx"])
                        .pick_file()
                {
                    self.open_any(ctx, &p);
                }
            }
            Action::Save => {
                if self.save() {
                    self.notify(ctx, "Saved");
                }
            }
            Action::SaveAs => {
                if self.save_as() {
                    self.notify(ctx, "Saved");
                }
            }
            Action::ExportPptx => self.export_pptx(ctx),
            Action::ExportPdf => self.export_pdf(ctx),
            Action::ImportPptx => {
                if self.confirm_discard()
                    && let Some(p) = rfd::FileDialog::new().add_filter("PowerPoint presentation", &["pptx"]).pick_file()
                {
                    self.import_pptx(ctx, &p);
                }
            }
            Action::OpenRecent(p) => {
                if !p.exists() {
                    self.recent.remove(&p);
                    self.error_dialog(&format!("{} no longer exists. It was removed from the recent list.", p.display()));
                } else if self.confirm_discard() {
                    self.open_any(ctx, &p);
                }
            }
            Action::ClearRecent => self.recent.clear(),
            Action::ExportPng => self.export_png(ctx),
            Action::Undo => {
                if self.history.undo(&mut self.doc) {
                    self.prune_selection();
                }
            }
            Action::Redo => {
                if self.history.redo(&mut self.doc) {
                    self.prune_selection();
                }
            }
            Action::Cut => {
                self.copy_selection(ctx);
                self.delete_selection();
            }
            Action::Copy => self.copy_selection(ctx),
            Action::Paste => self.paste_from_clipboard(ctx, None),
            Action::Delete => self.delete_selection(),
            Action::Duplicate => {
                let offset = self.camera.screen_delta_to_world(DVec2::new(24.0, 24.0));
                let copies: Vec<_> =
                    self.doc.elements.iter().filter(|e| self.selection.contains(&e.id)).cloned().collect();
                self.paste_elements(copies, offset);
            }
            Action::SelectAll => {
                self.selection = self.doc.elements.iter().filter(|e| !e.hidden).map(|e| e.id.clone()).collect();
            }
            Action::Deselect => {
                self.selection.clear();
                self.tool = Tool::Select;
            }
            Action::BringToFront | Action::SendToBack => {
                let (picked, rest): (Vec<_>, Vec<_>) =
                    self.doc.elements.drain(..).partition(|e| self.selection.contains(&e.id));
                self.doc.elements = if action == Action::BringToFront {
                    rest.into_iter().chain(picked).collect()
                } else {
                    picked.into_iter().chain(rest).collect()
                };
            }
            Action::SetTool(t) => self.tool = t,
            Action::InsertImage => {
                if let Some(files) = rfd::FileDialog::new().add_filter("Images and SVG", crate::media::IMAGE_EXTS).pick_files() {
                    self.import_image_files(ctx, &files, None);
                }
            }
            Action::InsertVideo => {
                if let Some(files) = rfd::FileDialog::new().add_filter("Videos", crate::media::VIDEO_EXTS).pick_files() {
                    self.import_video_files(ctx, &files, None);
                }
            }
            Action::FrameFromView => {
                self.frame_from_view();
            }
            Action::AddSelectedFrameToPath => {
                let frames: Vec<String> = self
                    .selection
                    .iter()
                    .filter(|id| self.doc.get(id).is_some_and(|e| e.kind.is_frame()))
                    .cloned()
                    .collect();
                for id in frames {
                    self.insert_step(&id);
                }
            }
            Action::GoToStep(i) => self.go_to_step(i),
            Action::MoveStep { from, to } => {
                if from < self.doc.path.len() {
                    let step = self.doc.path.remove(from);
                    let to = to.min(self.doc.path.len());
                    self.doc.path.insert(to, step);
                    self.current_step = Some(to);
                }
            }
            Action::DuplicateStep(i) => {
                if let Some(step) = self.doc.path.get(i).cloned() {
                    self.doc.path.insert(i + 1, step);
                    self.current_step = Some(i + 1);
                }
            }
            Action::RemoveStep(i) => {
                if i < self.doc.path.len() {
                    self.doc.path.remove(i);
                    self.current_step = match self.doc.path.len() {
                        0 => None,
                        n => Some(i.min(n - 1)),
                    };
                }
            }
            Action::Present { from_start } => {
                let start = if from_start { 0 } else { self.current_step.unwrap_or(0) };
                self.start_presenting(ctx, start, None);
            }
            Action::FormatText(f) => self.format_text(f),
            Action::PresenterView { from_start } => {
                let start = if from_start { 0 } else { self.current_step.unwrap_or(0) };
                self.start_presenting(ctx, start, Some(true));
            }
            Action::FitAll => self.fit_all(true),
            Action::FitSelection => self.fit_selection(),
            Action::ResetView => self.animate_to(Camera::default()),
            Action::ZoomIn => self.nav.zoom(viewport * 0.5, 2f64.ln()),
            Action::ZoomOut => self.nav.zoom(viewport * 0.5, -(2f64.ln())),
            Action::RotateView(deg) => self.nav.rotate(viewport * 0.5, deg.to_radians()),
            Action::Nudge(screen) => {
                let d = self.camera.screen_delta_to_world(screen);
                let ids = self.with_frame_contents(&self.selection.clone());
                for el in self.doc.elements.iter_mut().filter(|e| !e.locked) {
                    if ids.contains(&el.id) {
                        el.transform.set_position(el.transform.position() + d);
                    }
                }
            }
            Action::ToggleGrid => self.show_grid = !self.show_grid,
            Action::ToggleFrameOutlines => self.show_frames = !self.show_frames,
            Action::ToggleSlides => self.show_slides = !self.show_slides,
            Action::ToggleInspector => self.show_inspector = !self.show_inspector,
            Action::SetTheme(t) => ctx.set_theme(t),
            Action::ToggleShortcuts => self.show_shortcuts = !self.show_shortcuts,
            Action::ShowIcons => {
                self.icon_picker.open = true;
                self.icon_picker.replace = None;
            }
            Action::ShowTemplates => self.show_templates = true,
            Action::ShowLayouts => self.show_layouts = !self.show_layouts,
            Action::ToggleDesignCheck => self.design_check.open = !self.design_check.open,
            Action::InsertLayout(kind) => self.insert_layout(kind),
            Action::NewFromTemplate(t) => self.new_from_template(t),
            Action::ApplyTheme(i) => {
                if let Some(t) = glide_core::theme::builtin_themes().get(i) {
                    glide_core::theme::apply_theme(&mut self.doc, t);
                    self.notify(ctx, format!("Theme: {}", t.name));
                }
            }
            Action::Align(op) => {
                let ids = self.selection.clone();
                crate::design::align(&mut self.doc, &mut self.renderer.text, &ids, op, self.camera.rotation);
            }
            Action::InsertText(role) => self.insert_text_role(role),
            Action::ApplyTextRole(role) => {
                let theme = self.theme();
                for id in self.selection.clone() {
                    if let Some(ElementKind::Text(t)) = self.doc.get_mut(&id).map(|e| &mut e.kind) {
                        theme.apply_role(t, role);
                    }
                }
            }
            Action::SetShadow(shadow) => {
                for id in self.selection.clone() {
                    if let Some(el) = self.doc.get_mut(&id) {
                        el.shadow = shadow;
                    }
                }
            }
            Action::EditText => {
                if let [id] = self.selection.as_slice() {
                    let id = id.clone();
                    if matches!(self.doc.get(&id).map(|e| &e.kind), Some(ElementKind::Text(_))) {
                        self.start_text_edit(&id, None);
                    }
                }
            }
            Action::ToggleGradient => {
                let theme = self.theme();
                for id in self.selection.clone() {
                    let Some(el) = self.doc.get_mut(&id) else { continue };
                    let (fill, gradient) = match &mut el.kind {
                        ElementKind::Rect(r) => (&mut r.fill, &mut r.gradient),
                        ElementKind::Ellipse(e) => (&mut e.fill, &mut e.gradient),
                        _ => continue,
                    };
                    if gradient.is_some() {
                        *gradient = None;
                    } else {
                        let from = *fill.get_or_insert(theme.accents[0]);
                        let to = theme.accents.iter().copied().find(|c| *c != from).unwrap_or(theme.accents[1]);
                        *gradient = Some(glide_core::Gradient { to, angle: 45.0 });
                    }
                }
            }
        }
    }

    fn delete_selection(&mut self) {
        let sel = std::mem::take(&mut self.selection);
        for id in sel {
            if self.doc.get(&id).is_some_and(|e| !e.locked) {
                self.doc.remove(&id);
            }
        }
        if let Some(i) = self.current_step
            && i >= self.doc.path.len()
        {
            self.current_step = self.doc.path.len().checked_sub(1);
        }
    }

    /// The selection plus everything inside selected frames, so moving a frame
    /// carries the objects it groups.
    pub fn with_frame_contents(&mut self, ids: &[String]) -> Vec<String> {
        let mut out = ids.to_vec();
        let frames: Vec<Element> =
            ids.iter().filter_map(|id| self.doc.get(id)).filter(|e| e.kind.is_frame()).cloned().collect();
        for frame in frames {
            let size = frame.kind.intrinsic_size().unwrap_or_default();
            let inv = frame.transform.affine().inverse();
            for el in &self.doc.elements {
                if out.contains(&el.id) {
                    continue;
                }
                let local = inv.transform_point2(el.transform.position());
                if local.x.abs() <= size.x * 0.5 && local.y.abs() <= size.y * 0.5 {
                    out.push(el.id.clone());
                }
            }
        }
        out
    }

    /// Adds copies of elements (new ids), offset in world space, and selects them.
    pub fn paste_elements(&mut self, elements: Vec<Element>, offset: DVec2) {
        self.selection.clear();
        for mut el in elements {
            el.id.clear();
            el.locked = false;
            el.transform.set_position(el.transform.position() + offset);
            let id = self.doc.add(el);
            self.selection.push(id);
        }
    }

    fn prune_selection(&mut self) {
        let doc = &self.doc;
        self.selection.retain(|id| doc.get(id).is_some());
        if self.current_step.is_some_and(|i| i >= doc.path.len()) {
            self.current_step = doc.path.len().checked_sub(1);
        }
    }

    pub fn open_path(&mut self, p: &Path) {
        match Document::load(p) {
            Ok(doc) => {
                self.replace_document(doc, Some(p.to_path_buf()));
                self.recent.add(p);
            }
            Err(e) => self.error_dialog(&e.to_string()),
        }
    }

    /// Opens a GlideDeck document, or imports a PowerPoint file.
    pub fn open_any(&mut self, ctx: &egui::Context, p: &Path) {
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pptx")) {
            self.import_pptx(ctx, p);
        } else {
            self.open_path(p);
        }
    }

    /// Imports a PowerPoint file as a new, unsaved presentation.
    pub fn import_pptx(&mut self, ctx: &egui::Context, p: &Path) {
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                self.error_dialog(&format!("Could not read {}: {e}", p.display()));
                return;
            }
        };
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Presentation".into());
        let text = &mut self.renderer.text;
        match crate::pptx_import::import(&bytes, &stem, &mut |k| glide_render::local_size(text, k)) {
            Ok(mut imported) => {
                // Unsaved: pictures go to the session folder until Save As copies them.
                self.path = None;
                let base = self.base_dir();
                crate::pptx_import::store_media(&mut imported.doc, &imported.media, &base);
                let slides = imported.doc.path.len();
                self.replace_document(imported.doc, None);
                // Not saved yet: closing asks first.
                self.saved = Document::default();
                self.recent.add(p);
                let mut msg = format!("Imported {slides} slides from {}", p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
                if !imported.skipped.is_empty() {
                    msg.push_str(&format!(" · skipped {}", imported.skipped.join(", ")));
                }
                self.notify(ctx, msg);
            }
            Err(e) => self.error_dialog(&format!("Could not import {}: {e}", p.display())),
        }
    }

    fn replace_document(&mut self, doc: Document, path: Option<PathBuf>) {
        self.saved = doc.clone();
        self.history.reset(&doc);
        self.current_step = (!doc.path.is_empty()).then_some(0);
        self.doc = doc;
        self.path = path;
        self.selection.clear();
        self.drag = Drag::None;
        self.pending_fit = true;
        self.sync_base_dir();
    }

    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => match self.doc.save(&p) {
                Ok(()) => {
                    self.saved = self.doc.clone();
                    true
                }
                Err(e) => {
                    self.error_dialog(&e.to_string());
                    false
                }
            },
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let name = if self.doc.title.trim().is_empty() { "Untitled".to_owned() } else { self.doc.title.clone() };
        let Some(p) = rfd::FileDialog::new()
            .add_filter("GlideDeck document", &["json"])
            .set_file_name(format!("{name}.glide.json"))
            .save_file()
        else {
            return false;
        };
        self.save_to(p)
    }

    /// Saves to a new location, bringing referenced media along so relative
    /// paths keep working.
    pub fn save_to(&mut self, p: PathBuf) -> bool {
        let old_base = self.base_dir();
        let new_base = p.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_default();
        match assets::relocate(&self.doc, &old_base, &new_base) {
            Ok(missing) if !missing.is_empty() => {
                self.error_dialog(&format!("Some media files could not be found and were not copied:\n{}", missing.join("\n")));
            }
            Err(e) => {
                self.error_dialog(&format!("Could not copy media next to the document: {e}"));
                return false;
            }
            Ok(_) => {}
        }
        self.path = Some(p.clone());
        self.sync_base_dir();
        let ok = self.save();
        if ok {
            self.recent.add(&p);
        }
        ok
    }

    fn export_pptx(&mut self, ctx: &egui::Context) {
        let name = if self.doc.title.trim().is_empty() { "Presentation".to_owned() } else { self.doc.title.clone() };
        let Some(out) = rfd::FileDialog::new()
            .add_filter("PowerPoint presentation", &["pptx"])
            .set_file_name(format!("{name}.pptx"))
            .save_file()
        else {
            return;
        };
        let base = self.base_dir();
        match glide_pptx::export(&self.doc, Some(&base), &mut self.renderer.text, &out) {
            Ok(r) => {
                let mut msg = format!("Exported {} slides to {}", r.slides, out.display());
                if !r.skipped.is_empty() {
                    msg.push_str(&format!(" ({} media skipped)", r.skipped.len()));
                }
                self.notify(ctx, msg);
            }
            Err(e) => self.error_dialog(&format!("PowerPoint export failed: {e}")),
        }
    }

    fn export_pdf(&mut self, ctx: &egui::Context) {
        if self.doc.path.is_empty() {
            self.notify(ctx, "Add a frame first — every frame on the path is a page");
            return;
        }
        let name = if self.doc.title.trim().is_empty() { "Presentation".to_owned() } else { self.doc.title.clone() };
        let Some(out) = rfd::FileDialog::new().add_filter("PDF document", &["pdf"]).set_file_name(format!("{name}.pdf")).save_file() else {
            return;
        };
        match crate::pdf::export(&mut self.renderer, &self.doc, &out, crate::pdf::PdfOptions::default()) {
            Ok(r) => self.notify(ctx, format!("Exported {} pages to {}", r.pages, out.display())),
            Err(e) => self.error_dialog(&format!("PDF export failed: {e}")),
        }
    }

    fn export_png(&mut self, ctx: &egui::Context) {
        let Some(out) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name("view.png").save_file()
        else {
            return;
        };
        let scale = 2.0;
        let vp = self.viewport();
        let size = [(vp.x * scale) as u32, (vp.y * scale) as u32];
        let mut cam = self.camera;
        cam.zoom *= scale;
        match self.renderer.snapshot(&self.doc, &cam, size, false) {
            Ok(rgba) => match image::save_buffer(&out, &rgba, size[0], size[1], image::ExtendedColorType::Rgba8) {
                Ok(()) => self.notify(ctx, format!("Saved {}", out.display())),
                Err(e) => self.error_dialog(&e.to_string()),
            },
            Err(e) => self.error_dialog(&e.to_string()),
        }
    }

    /// Returns true when it is fine to drop the current document.
    pub(crate) fn confirm_discard(&mut self) -> bool {
        // Native dialogs would block headless UI tests.
        if !self.is_dirty() || cfg!(test) {
            return true;
        }
        let answer = rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description(format!("Save changes to “{}” before continuing?", self.display_name()))
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .set_level(rfd::MessageLevel::Warning)
            .show();
        match answer {
            rfd::MessageDialogResult::Yes => self.save(),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }

    pub fn error_dialog(&self, msg: &str) {
        log::error!("{msg}");
        if cfg!(test) {
            return;
        }
        rfd::MessageDialog::new().set_title("GlideDeck").set_description(msg).set_level(rfd::MessageLevel::Error).show();
    }

    pub fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            None if !self.doc.title.is_empty() => self.doc.title.clone(),
            None => "Untitled".into(),
        }
    }

    // --- input --------------------------------------------------------------------

    fn collect_shortcuts(&mut self, ctx: &egui::Context) -> Vec<Action> {
        let mut actions = Vec::new();
        let typing = ctx.text_edit_focused() || self.text_edit.is_some();
        ctx.input_mut(|i| {
            // File shortcuts work even while a text field has focus.
            for (s, a) in [
                (SC_SAVE_AS, Action::SaveAs),
                (SC_SAVE, Action::Save),
                (SC_OPEN, Action::Open),
                (SC_NEW, Action::New),
                (SC_PRESENTER, Action::PresenterView { from_start: false }),
                (SC_PRESENT_HERE, Action::Present { from_start: false }),
                (SC_PRESENT, Action::Present { from_start: true }),
            ] {
                if i.consume_shortcut(&s) {
                    actions.push(a);
                }
            }
            if typing {
                return;
            }
            // Clipboard: egui turns the platform shortcuts into events.
            for ev in &i.events {
                match ev {
                    egui::Event::Copy => actions.push(Action::Copy),
                    // Ctrl/⌘+Shift+X arrives as a cut too: strikethrough.
                    egui::Event::Cut if i.modifiers.shift => actions.push(Action::FormatText(crate::textformat::TextFormat::Strike)),
                    egui::Event::Cut => actions.push(Action::Cut),
                    _ => {}
                }
            }
            use crate::textformat::TextFormat as F;
            for (s, f) in [
                (SC_BOLD, F::Bold),
                (SC_ITALIC, F::Italic),
                (SC_UNDERLINE, F::Underline),
                (SC_BIGGER, F::Bigger),
                (SC_SMALLER, F::Smaller),
                (SC_CLEAR_FORMAT, F::Clear),
            ] {
                if i.consume_shortcut(&s) {
                    actions.push(Action::FormatText(f));
                }
            }
            let table = [
                (SC_REDO, Action::Redo),
                (SC_UNDO, Action::Undo),
                (SC_REDO_ALT, Action::Redo),
                (SC_DUPLICATE, Action::Duplicate),
                (SC_SELECT_ALL, Action::SelectAll),
                (SC_FRONT, Action::BringToFront),
                (SC_BACK, Action::SendToBack),
                (SC_RESET_VIEW, Action::ResetView),
                (SC_FIT_ALL, Action::FitAll),
                (SC_FIT_SELECTION, Action::FitSelection),
                (SC_ZOOM_IN, Action::ZoomIn),
                (SC_ZOOM_OUT, Action::ZoomOut),
                (SC_FRAME_VIEW, Action::FrameFromView),
                (SC_INSERT_IMAGE, Action::InsertImage),
                (SC_ICONS, Action::ShowIcons),
                (SC_LAYOUTS, Action::ShowLayouts),
            ];
            for (s, a) in table {
                if i.consume_shortcut(&s) {
                    actions.push(a);
                }
            }
            let plain = [
                (Key::Delete, Action::Delete),
                (Key::Backspace, Action::Delete),
                (Key::Escape, Action::Deselect),
                (Key::Home, Action::FitAll),
                (Key::G, Action::ToggleGrid),
                (Key::F1, Action::ToggleShortcuts),
                (Key::F2, Action::EditText),
                (Key::Enter, Action::EditText),
                (Key::OpenBracket, Action::RotateView(-15.0)),
                (Key::CloseBracket, Action::RotateView(15.0)),
            ];
            for (k, a) in plain {
                if i.consume_key(Modifiers::NONE, k) {
                    actions.push(a);
                }
            }
            for t in Tool::ALL {
                if i.consume_key(Modifiers::NONE, t.key()) {
                    actions.push(Action::SetTool(t));
                }
            }
            if i.consume_key(Modifiers::NONE, Key::PageDown) {
                let next = self.current_step.map(|s| s + 1).unwrap_or(0);
                if next < self.doc.path.len() {
                    actions.push(Action::GoToStep(next));
                }
            }
            if i.consume_key(Modifiers::NONE, Key::PageUp)
                && let Some(s) = self.current_step
                && s > 0
            {
                actions.push(Action::GoToStep(s - 1));
            }
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            for (k, d) in [
                (Key::ArrowLeft, DVec2::new(-step, 0.0)),
                (Key::ArrowRight, DVec2::new(step, 0.0)),
                (Key::ArrowUp, DVec2::new(0.0, -step)),
                (Key::ArrowDown, DVec2::new(0.0, step)),
            ] {
                if i.key_pressed(k) && !self.selection.is_empty() {
                    actions.push(Action::Nudge(d));
                }
            }
        });
        actions
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!("{}{} — GlideDeck", if self.is_dirty() { "● " } else { "" }, self.display_name());
        if title != self.title_shown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_shown = title;
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.confirm_discard() {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Renders the canvas into the offscreen target and returns the texture.
    pub fn render_canvas(&mut self, ui: &egui::Ui, frame: &eframe::Frame, rect: egui::Rect, editor: bool) -> Option<egui::TextureId> {
        let rs = frame.wgpu_render_state()?;
        let ppp = ui.ctx().pixels_per_point();
        let px = [
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        ];
        if self.target.as_ref().is_none_or(|t| t.size != px) {
            let target = RenderTarget::new(&rs.device, px);
            let mut egui_renderer = rs.renderer.write();
            match self.texture_id {
                Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    &target.unorm_view,
                    wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    self.texture_id =
                        Some(egui_renderer.register_native_texture(&rs.device, &target.unorm_view, wgpu::FilterMode::Linear))
                }
            }
            self.target = Some(target);
        }
        let target = self.target.as_ref().expect("target ensured above");
        let opts = glide_render::RenderOptions { show_grid: editor && self.show_grid, pixels_per_point: ppp as f64 };
        self.stats = self.renderer.render(target, &self.doc, &self.camera, self.viewport(), &opts);
        if self.renderer.any_video_playing() {
            ui.ctx().request_repaint();
        } else if self.renderer.poll_images() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
        }
        self.texture_id
    }

    /// Renders the document from `camera` into a separate texture of `size`
    /// points, for previews shown next to the main view.
    pub fn render_snapshot(&mut self, ui: &egui::Ui, frame: &eframe::Frame, key: &'static str, size: egui::Vec2, camera: &Camera) -> Option<egui::TextureId> {
        let rs = frame.wgpu_render_state()?;
        let ppp = ui.ctx().pixels_per_point();
        let px = [(size.x * ppp).round().max(1.0) as u32, (size.y * ppp).round().max(1.0) as u32];
        if self.snapshots.get(key).is_none_or(|(t, _)| t.size != px) {
            let target = RenderTarget::new(&rs.device, px);
            let mut egui_renderer = rs.renderer.write();
            let id = match self.snapshots.get(key) {
                Some((_, id)) => {
                    egui_renderer.update_egui_texture_from_wgpu_texture(&rs.device, &target.unorm_view, wgpu::FilterMode::Linear, *id);
                    *id
                }
                None => egui_renderer.register_native_texture(&rs.device, &target.unorm_view, wgpu::FilterMode::Linear),
            };
            self.snapshots.insert(key, (target, id));
        }
        let (target, id) = self.snapshots.get(key)?;
        let opts = glide_render::RenderOptions { show_grid: false, pixels_per_point: ppp as f64 };
        self.renderer.render(target, &self.doc, camera, DVec2::new(size.x as f64, size.y as f64), &opts);
        Some(*id)
    }
}

impl eframe::App for GlideDeckApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close(&ctx);
        self.handle_dropped_files(&ctx);

        if let Some(p) = self.import_on_start.take() {
            self.import_pptx(&ctx, &p);
        }
        if let Some(show) = self.start_show.take() {
            self.start_presenting(&ctx, show.step, show.presenter);
        }
        if self.presenting.is_some() {
            crate::present::present_ui(self, ui, frame);
            self.update_title(&ctx);
            return;
        }

        let mut actions = self.collect_shortcuts(&ctx);
        actions.extend(self.handle_paste_events(&ctx));
        let p = theme::palette(&ctx);

        egui::Panel::top("ribbon")
            .frame(egui::Frame::new().fill(p.surface).inner_margin(egui::Margin { left: 10, right: 10, top: 6, bottom: 0 }))
            .show_separator_line(false)
            .show(ui, |ui| actions.extend(crate::ribbon::ribbon(self, ui)));
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(p.surface).inner_margin(egui::Margin::symmetric(12, 4)))
            .show_separator_line(false)
            .show(ui, |ui| crate::ribbon::status_bar(self, ui));
        if self.show_slides {
            egui::Panel::left("slides")
                .frame(egui::Frame::new().fill(p.surface).inner_margin(egui::Margin::symmetric(10, 10)))
                .show_separator_line(false)
                .resizable(true)
                .default_size(232.0)
                .size_range(180.0..=380.0)
                .show(ui, |ui| actions.extend(crate::slides::slides_panel(self, ui, frame)));
        }
        if self.show_inspector {
            egui::Panel::right("inspector")
                .frame(egui::Frame::new().fill(p.surface).inner_margin(egui::Margin::symmetric(14, 10)))
                .show_separator_line(false)
                .resizable(true)
                .default_size(300.0)
                .size_range(260.0..=440.0)
                .show(ui, |ui| actions.extend(crate::inspector::inspector(self, ui)));
        }
        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(p.app_bg).inner_margin(egui::Margin::same(8)))
            .show(ui, |ui| actions.extend(self.canvas_ui(ui, frame)));

        for a in actions {
            self.perform(&ctx, a);
        }
        if self.show_shortcuts {
            crate::ribbon::shortcuts_window(self, &ctx);
        }
        crate::galleries::icon_picker(self, &ctx);
        crate::galleries::design_check_window(self, &ctx);
        let mut more = crate::galleries::template_gallery(self, &ctx, frame);
        more.extend(crate::galleries::layout_gallery(self, &ctx, frame));
        for a in more {
            self.perform(&ctx, a);
        }

        let busy = ctx.input(|i| i.pointer.any_down()) || ctx.text_edit_focused() || self.text_edit.is_some();
        self.history.track(&self.doc, busy);
        self.update_title(&ctx);
        if self.toast.as_ref().is_some_and(|(_, until)| ctx.input(|i| i.time) < *until) {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

pub fn format_zoom(zoom: f64) -> String {
    let pct = zoom * 100.0;
    if pct >= 1e6 || pct < 0.01 {
        format!("{pct:.1e}%")
    } else if pct >= 10.0 {
        format!("{pct:.0}%")
    } else {
        format!("{pct:.2}%")
    }
}

fn title_from_path(p: &Path) -> String {
    let name = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    name.trim_end_matches(".json").trim_end_matches(".glide").trim_end_matches(".zui").to_owned()
}
