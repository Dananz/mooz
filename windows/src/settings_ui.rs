//! The Settings window: General, Applications, About. Runs as its own process
//! (`Mooz.exe --settings`) so closing it never touches the tray app. Every
//! change is written to config.json at once; the tray app and the hook thread
//! pick it up from the file, as the macOS app does with UserDefaults.

use crate::core::config::{
    config_path, AppEntry, ConfigWatcher, InputSource, ListMode, ZoomMethod, ZoomStyle, SENSITIVITY_MAX,
    SENSITIVITY_MIN,
};
use crate::core::modifiers::Modifiers;
use crate::platform;
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Frame, Layout,
    Margin, RichText, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2,
};
use std::sync::Arc;
use std::time::Duration;

pub const WINDOW_TITLE: &str = "Mooz Settings";
const VERSION: &str = env!("MOOZ_VERSION");
const ICON_RGBA: &[u8] = include_bytes!("../assets/icon-128.rgba");
const ICON_SIZE: u32 = 128;

const BG: Color32 = Color32::from_rgb(0x15, 0x15, 0x17);
const CARD: Color32 = Color32::from_rgb(0x1F, 0x1F, 0x22);
const CARD_BORDER: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2F);
const CONTROL_BG: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x30);
const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF4);
const MUTED: Color32 = Color32::from_rgb(0x9A, 0x9A, 0xA1);
const ACCENT: Color32 = Color32::from_rgb(0xFF, 0xB0, 0x20);
const ON_ACCENT: Color32 = Color32::from_rgb(0x1C, 0x14, 0x00);

const PAGE_WIDTH: f32 = 500.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Applications,
    About,
}

pub fn run() -> eframe::Result {
    let icon = egui::IconData { rgba: ICON_RGBA.to_vec(), width: ICON_SIZE, height: ICON_SIZE };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size([PAGE_WIDTH + 40.0, 700.0])
            .with_min_inner_size([PAGE_WIDTH + 40.0, 420.0])
            .with_icon(Arc::new(icon)),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(WINDOW_TITLE, options, Box::new(|cc| Ok(Box::new(SettingsApp::new(cc)))))
}

struct SettingsApp {
    watcher: ConfigWatcher,
    tab: Tab,
    autostart: bool,
    running: Vec<AppEntry>,
    typed_exe: String,
    icon: TextureHandle,
    save_error: Option<String>,
    pinch_available: bool,
    #[cfg(not(windows))]
    preview: preview_shot::PreviewShot,
}

impl SettingsApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        style(&cc.egui_ctx);
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [ICON_SIZE as usize, ICON_SIZE as usize],
            ICON_RGBA,
        );
        let icon = cc.egui_ctx.load_texture("mooz-icon", image, egui::TextureOptions::LINEAR);
        Self {
            watcher: ConfigWatcher::new(config_path()),
            tab: Tab::General,
            autostart: platform::autostart_enabled(),
            running: platform::running_apps(),
            typed_exe: String::new(),
            icon,
            save_error: None,
            pinch_available: platform::native_pinch_available(),
            #[cfg(not(windows))]
            preview: preview_shot::PreviewShot::from_env(),
        }
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(not(windows))]
        if let Some(tab) = self.preview.step(ctx) {
            self.tab = tab;
        }
        // The tray menu can flip "Enabled" while this window is open.
        self.watcher.refresh();
        ctx.request_repaint_after(Duration::from_secs(1));
        let before = self.watcher.config.clone();

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(20, 16)))
            .show(ctx, |ui| {
                tab_bar(ui, &mut self.tab);
                ui.add_space(14.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match self.tab {
                        Tab::General => self.general(ui),
                        Tab::Applications => self.applications(ui),
                        Tab::About => self.about(ui),
                    }
                    if let Some(err) = &self.save_error {
                        ui.add_space(8.0);
                        ui.label(RichText::new(err).color(Color32::from_rgb(0xFF, 0x6B, 0x6B)));
                    }
                });
            });

        if self.watcher.config != before {
            self.save_error = self
                .watcher
                .save()
                .err()
                .map(|e| format!("Couldn't save settings: {e}"));
        }
    }
}

// MARK: - Tabs

impl SettingsApp {
    fn general(&mut self, ui: &mut Ui) {
        let c = &mut self.watcher.config;

        card(ui, |ui| {
            row(ui, "Enable Mooz", Some("Emulate trackpad pinch-to-zoom with your mouse."), |ui| {
                toggle(ui, &mut c.enabled);
            });
        });

        section(ui, "Gesture");
        card(ui, |ui| {
            row(ui, "Trigger keys", Some("Hold these while you move the mouse or scroll."), |_| {});
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for (m, name) in Modifiers::ORDERED {
                    let on = c.modifiers.contains(m);
                    if chip(ui, name, on).clicked() {
                        let next = c.modifiers.toggled(m);
                        // Keep at least one key: an empty combo would never zoom.
                        if !next.is_empty() {
                            c.modifiers = next;
                        }
                    }
                }
            });
            divider(ui);
            row(ui, "Input source", None, |ui| {
                segmented(
                    ui,
                    &mut c.input_source,
                    &[(InputSource::MouseDrag, "Mouse Drag"), (InputSource::ScrollWheel, "Scroll Wheel")],
                );
            });
            divider(ui);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sensitivity").color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.1}×", c.sensitivity)).color(MUTED));
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Slower").small().color(MUTED));
                let width = ui.available_width() - 48.0;
                slider(ui, &mut c.sensitivity, SENSITIVITY_MIN..=SENSITIVITY_MAX, 0.1, width);
                ui.label(RichText::new("Faster").small().color(MUTED));
            });
        });
        hint(ui, &usage_sentence(c.modifiers, c.input_source));

        section(ui, "Behavior");
        card(ui, |ui| {
            row(
                ui,
                "Anchor cursor while zooming",
                Some("Locks the pointer in place during a zoom so the page doesn't pan. Turn off to let the cursor move freely."),
                |ui| {
                    toggle(ui, &mut c.anchor_cursor);
                },
            );
            divider(ui);
            let method_note = zoom_method_note(c.zoom_method, self.pinch_available);
            row(ui, "Zoom method", Some(method_note), |ui| {
                segmented(ui, &mut c.zoom_method, &[(ZoomMethod::Pinch, "Pinch"), (ZoomMethod::Wheel, "Ctrl + Wheel")]);
            });
            if c.zoom_method == ZoomMethod::Wheel || !self.pinch_available {
                divider(ui);
                row(ui, "Wheel zoom", Some(zoom_style_note(c.zoom_style)), |ui| {
                    segmented(ui, &mut c.zoom_style, &[(ZoomStyle::Steps, "Steps"), (ZoomStyle::Smooth, "Smooth")]);
                });
            }
        });

        section(ui, "System");
        card(ui, |ui| {
            row(ui, "Start with Windows", Some("Launch Mooz in the tray when you sign in."), |ui| {
                let mut on = self.autostart;
                if toggle(ui, &mut on).changed() {
                    match platform::set_autostart(on) {
                        Ok(()) => self.autostart = on,
                        Err(e) => self.save_error = Some(format!("Couldn't change startup: {e}")),
                    }
                }
            });
        });
    }

    fn applications(&mut self, ui: &mut Ui) {
        let c = &mut self.watcher.config;

        card(ui, |ui| {
            row(ui, "List mode", Some(list_mode_note(c.list_mode)), |ui| {
                segmented(ui, &mut c.list_mode, &[(ListMode::Blocklist, "Blocklist"), (ListMode::Allowlist, "Allowlist")]);
            });
        });

        section(ui, match c.list_mode {
            ListMode::Blocklist => "Mooz is off in",
            ListMode::Allowlist => "Mooz only works in",
        });
        card(ui, |ui| {
            if c.apps.is_empty() {
                ui.label(RichText::new("No apps yet. Add one below.").color(MUTED));
            }
            let mut remove = None;
            for (i, app) in c.apps.iter().enumerate() {
                if i > 0 {
                    divider(ui);
                }
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&app.name).color(TEXT));
                        ui.label(RichText::new(&app.exe).small().color(MUTED));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if link(ui, "Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                c.apps.remove(i);
            }
        });

        section(ui, "Add an app");
        card(ui, |ui| {
            let available: Vec<AppEntry> = self
                .running
                .iter()
                .filter(|r| !c.apps.iter().any(|a| a.exe.eq_ignore_ascii_case(&r.exe)))
                .cloned()
                .collect();
            ui.horizontal(|ui| {
                let mut picked = None;
                egui::ComboBox::from_id_salt("running-apps")
                    .width(ui.available_width() - 80.0)
                    .selected_text(RichText::new("Choose a running app").color(MUTED))
                    .show_ui(ui, |ui| {
                        if available.is_empty() {
                            ui.label(RichText::new("No other apps with open windows").color(MUTED));
                        }
                        for app in &available {
                            let label = format!("{}  ({})", app.name, app.exe);
                            if ui.selectable_label(false, label).clicked() {
                                picked = Some(app.clone());
                            }
                        }
                    });
                if link(ui, "Refresh").clicked() {
                    self.running = platform::running_apps();
                }
                if let Some(app) = picked {
                    c.apps.push(app);
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let field = egui::TextEdit::singleline(&mut self.typed_exe)
                    .hint_text("or type an .exe name, e.g. figma.exe")
                    .desired_width(ui.available_width() - 80.0);
                let response = ui.add(field);
                let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (link(ui, "Add").clicked() || submitted) && !self.typed_exe.trim().is_empty() {
                    let mut exe = self.typed_exe.trim().to_lowercase();
                    if !exe.ends_with(".exe") {
                        exe.push_str(".exe");
                    }
                    if !c.apps.iter().any(|a| a.exe.eq_ignore_ascii_case(&exe)) {
                        let name = exe.trim_end_matches(".exe").to_string();
                        c.apps.push(AppEntry { exe, name });
                    }
                    self.typed_exe.clear();
                }
            });
        });
        hint(ui, "Mooz checks the app in front (the active window).");
    }

    fn about(&mut self, ui: &mut Ui) {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            ui.add(egui::Image::new(&self.icon).fit_to_exact_size(Vec2::splat(96.0)));
            ui.add_space(8.0);
            ui.label(RichText::new("Mooz").size(24.0).strong().color(TEXT));
            ui.label(RichText::new("Trackpad pinch-to-zoom for any mouse.").color(MUTED));
            ui.label(RichText::new(format!("Version {VERSION}")).small().color(MUTED));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                // Center the row of links.
                let width = 230.0;
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                if link(ui, "Website").clicked() {
                    platform::open_url("https://mooz.dananz.com/");
                }
                ui.label(RichText::new("·").color(MUTED));
                if link(ui, "GitHub").clicked() {
                    platform::open_url("https://github.com/Dananz/mooz");
                }
                ui.label(RichText::new("·").color(MUTED));
                if link(ui, "Releases").clicked() {
                    platform::open_url("https://github.com/Dananz/mooz/releases");
                }
            });
        });

        section(ui, "Good to know");
        card(ui, |ui| {
            note(ui, "Mooz zooms with a two-finger pinch on a virtual precision touchpad, the same signal a real touchpad sends, so browsers and other apps zoom smoothly at the cursor. Windows Settings, Touchpad, Pinch to zoom must stay on.");
            divider(ui);
            note(ui, "On Windows versions without touchpad injection (Windows 10, older Windows 11), the pinch goes on a virtual touchscreen around the cursor instead: the same smooth zoom in browsers. If that fails too, Mooz sends Ctrl + mouse wheel, which zooms the active window in the app's own steps.");
            divider(ui);
            note(ui, "Windows blocks input into apps running as administrator. To zoom in those, run Mooz as administrator too.");
            divider(ui);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Settings file").color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if link(ui, "Show").clicked() {
                        platform::reveal(self.watcher.path());
                    }
                });
            });
            ui.label(RichText::new(self.watcher.path().display().to_string()).small().color(MUTED));
            divider(ui);
            let log_path = std::env::temp_dir().join("mooz-debug.log");
            row(ui, "Debug log", Some("Records what Mooz sees and sends, to help track down problems."), |ui| {
                toggle(ui, &mut self.watcher.config.debug);
                if log_path.exists() && link(ui, "Show").clicked() {
                    platform::reveal(&log_path);
                }
            });
        });
    }
}

// MARK: - Copy

fn usage_sentence(modifiers: Modifiers, source: InputSource) -> String {
    let keys = modifiers.label();
    match source {
        InputSource::MouseDrag => format!("Hold {keys} and move the mouse up to zoom in, down to zoom out."),
        InputSource::ScrollWheel => format!("Hold {keys} and scroll to zoom."),
    }
}

fn zoom_method_note(method: ZoomMethod, pinch_available: bool) -> &'static str {
    match (method, pinch_available) {
        (ZoomMethod::Pinch, true) => "A virtual two-finger pinch: smooth zoom right where the cursor is, like a real touchpad.",
        (ZoomMethod::Pinch, false) => "This version of Windows can't host a virtual touchpad or touchscreen, so Mooz uses Ctrl + wheel.",
        (ZoomMethod::Wheel, _) => "Sends Ctrl + mouse wheel. Works everywhere, but apps zoom in their own steps.",
    }
}

fn zoom_style_note(style: ZoomStyle) -> &'static str {
    match style {
        ZoomStyle::Steps => "One zoom level at a time. Works the same in every app.",
        ZoomStyle::Smooth => "Finer, continuous zoom where the app supports it. Some apps jump a full level per tick.",
    }
}

fn list_mode_note(mode: ListMode) -> &'static str {
    match mode {
        ListMode::Blocklist => "Mooz works everywhere except the apps below.",
        ListMode::Allowlist => "Mooz works only in the apps below.",
    }
}

// MARK: - Components

fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(16.0);
    ui.label(RichText::new(title).small().strong().color(MUTED));
    ui.add_space(4.0);
}

fn hint(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).small().color(MUTED));
}

fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).color(MUTED)).wrap());
}

fn divider(ui: &mut Ui) {
    ui.add_space(8.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(1.0_f32, CARD_BORDER));
    ui.add_space(9.0);
}

/// Title (and optional wrapped subtitle) on the left, a control on the right.
fn row(ui: &mut Ui, title: &str, subtitle: Option<&str>, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let text_width = (ui.available_width() - 190.0).max(160.0);
        ui.allocate_ui_with_layout(Vec2::new(text_width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(text_width);
            ui.label(RichText::new(title).color(TEXT));
            if let Some(subtitle) = subtitle {
                ui.add(egui::Label::new(RichText::new(subtitle).small().color(MUTED)).wrap());
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
}

/// A thin amber track with a white knob, matching the toggle switches.
/// Drag or click to set; arrow keys step when focused.
fn slider(
    ui: &mut Ui,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
    width: f32,
) -> egui::Response {
    const KNOB: f32 = 9.0;
    let (min, max) = (*range.start(), *range.end());
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, 2.0 * KNOB + 4.0), Sense::click_and_drag());
    // Keep the knob inside the widget at both ends.
    let track = rect.shrink2(Vec2::new(KNOB, 0.0));

    let snap = |v: f64| ((v / step).round() * step).clamp(min, max);
    let mut next = *value;
    if let Some(pos) = response.interact_pointer_pos() {
        let t = f64::from(((pos.x - track.left()) / track.width()).clamp(0.0, 1.0));
        next = snap(min + t * (max - min));
    }
    if response.has_focus() {
        let (left, right) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
        if left {
            next = snap(*value - step);
        }
        if right {
            next = snap(*value + step);
        }
    }
    if (next - *value).abs() > f64::EPSILON {
        *value = next;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let t = ((*value - min) / (max - min)) as f32;
        let x = egui::lerp(track.left()..=track.right(), t);
        let y = rect.center().y;
        let bar = |from: f32, to: f32| egui::Rect::from_min_max(egui::pos2(from, y - 2.0), egui::pos2(to, y + 2.0));
        painter.rect_filled(bar(track.left(), track.right()), 2.0, CONTROL_BG);
        painter.rect_filled(bar(track.left(), x), 2.0, ACCENT);

        let active = response.hovered() || response.dragged();
        let radius = if response.dragged() { KNOB + 1.0 } else { KNOB };
        let center = egui::pos2(x, y);
        painter.circle_filled(center + Vec2::new(0.0, 1.0), radius + 1.0, Color32::from_black_alpha(70));
        painter.circle_filled(center, radius, Color32::WHITE);
        if active {
            painter.circle_stroke(center, radius + 3.0, Stroke::new(3.0_f32, ACCENT.gamma_multiply(0.25)));
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = Vec2::new(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let fill = lerp_color(CONTROL_BG, ACCENT, t);
        let radius = rect.height() / 2.0;
        ui.painter().rect(rect, radius, fill, Stroke::NONE, StrokeKind::Inside);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        ui.painter().circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, Color32::WHITE);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn segmented<T: Copy + PartialEq>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> egui::Response {
    Frame::new()
        .fill(CONTROL_BG)
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            // `horizontal` shrinks to its content, so the pill hugs its options.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                // Inside a right-to-left row the options are placed from the
                // right, so feed them reversed to keep reading order.
                let reversed = ui.layout().prefer_right_to_left();
                let ordered: Vec<_> = if reversed {
                    options.iter().rev().collect()
                } else {
                    options.iter().collect()
                };
                for (option, label) in ordered {
                    let selected = *value == *option;
                    let text = RichText::new(*label)
                        .size(12.5)
                        .color(if selected { ON_ACCENT } else { MUTED });
                    let button = egui::Button::new(text)
                        .fill(if selected { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(12))
                        .min_size(Vec2::new(0.0, 24.0));
                    if ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        *value = *option;
                    }
                }
            });
        })
        .response
}

fn chip(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    let text = RichText::new(label).color(if on { ON_ACCENT } else { TEXT });
    let button = egui::Button::new(text)
        .fill(if on { ACCENT } else { CONTROL_BG })
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(8))
        .min_size(Vec2::new(64.0, 30.0));
    ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn link(ui: &mut Ui, label: &str) -> egui::Response {
    let response = ui.add(
        egui::Label::new(RichText::new(label).color(ACCENT)).sense(Sense::click()),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn tab_bar(ui: &mut Ui, tab: &mut Tab) {
    // Center using last frame's measured width (egui lays out in one pass).
    let id = ui.id().with("tab-bar-width");
    let width = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(280.0);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        let response = segmented(
            ui,
            tab,
            &[(Tab::General, "General"), (Tab::Applications, "Applications"), (Tab::About, "About")],
        );
        let measured = response.rect.width();
        if (measured - width).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id, measured));
            ui.ctx().request_repaint();
        }
    });
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

// MARK: - Look

fn style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke.color = TEXT;
    visuals.widgets.inactive.fg_stroke.color = TEXT;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0_f32, ON_ACCENT);
    visuals.widgets.inactive.weak_bg_fill = CONTROL_BG;
    visuals.widgets.inactive.bg_fill = CONTROL_BG;
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);
    visuals.extreme_bg_color = Color32::from_rgb(0x19, 0x19, 0x1C);
    ctx.set_visuals(visuals);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(12.0, 4.0);
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
}

/// Uses Segoe UI on Windows (falls back to egui's bundled font elsewhere).
fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\segoeui.ttf",
        "/System/Library/Fonts/SFNS.ttf",
    ];
    let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) else { return };
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("system-ui".into(), Arc::new(FontData::from_owned(bytes)));
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "system-ui".into());
    ctx.set_fonts(fonts);
}

/// Preview-only (non-Windows): `MOOZ_PREVIEW_SHOT=/tmp/shot MOOZ_PREVIEW_TAB=about`
/// renders a tab, writes `<path>.rgba` plus `<path>.size`, and exits. Lets the
/// window be checked without a Windows machine or any synthetic input.
#[cfg(not(windows))]
mod preview_shot {
    use super::Tab;
    use eframe::egui;

    pub struct PreviewShot {
        path: Option<String>,
        tab: Tab,
        frames: u32,
    }

    impl PreviewShot {
        pub fn from_env() -> Self {
            let tab = match std::env::var("MOOZ_PREVIEW_TAB").as_deref() {
                Ok("applications") => Tab::Applications,
                Ok("about") => Tab::About,
                _ => Tab::General,
            };
            Self { path: std::env::var("MOOZ_PREVIEW_SHOT").ok(), tab, frames: 0 }
        }

        /// Returns the tab to force while a shot is pending.
        pub fn step(&mut self, ctx: &egui::Context) -> Option<Tab> {
            let path = self.path.clone()?;
            self.frames += 1;
            ctx.request_repaint();
            if self.frames == 10 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(image) = shot {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let _ = std::fs::write(format!("{path}.rgba"), bytes);
                let _ = std::fs::write(format!("{path}.size"), format!("{} {}", image.size[0], image.size[1]));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(self.tab)
        }
    }
}
