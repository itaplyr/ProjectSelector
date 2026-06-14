use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui::{self, Vec2};
use crate::projects;

const BG: egui::Color32 = egui::Color32::from_rgb(0x1f, 0x1f, 0x21);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(0x24, 0x24, 0x26);
const TEXT: egui::Color32 = egui::Color32::from_rgb(0x92, 0x90, 0x92);

const MUTED: egui::Color32 = egui::Color32::from_rgb(0x6c, 0x70, 0x86);
const BLUE: egui::Color32 = egui::Color32::from_rgb(0x89, 0xb4, 0xfa);
const YELLOW: egui::Color32 = egui::Color32::from_rgb(0xf9, 0xe2, 0xaf);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Personal,
    Customers,
}

pub struct ProjectApp {
    projects: Vec<projects::Project>,
    selected: usize,
    search_query: String,
    mode: Mode,
    needs_reload: bool,
    was_focused: bool,
    project_rx: Option<mpsc::Receiver<Vec<projects::Project>>>,
    confirm_delete: Option<usize>,
    confirm_choice: usize,
    app_start: std::time::Instant,
    logged_first_frame: bool,
}

impl Default for ProjectApp {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            selected: 0,
            search_query: String::new(),
            mode: Mode::Personal,
            needs_reload: false,
            was_focused: false,
            project_rx: None,
            confirm_delete: None,
            confirm_choice: 0,
            app_start: std::time::Instant::now(),
            logged_first_frame: false,
        }
    }
}

fn time_ago(t: std::time::SystemTime) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let then = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = now - then;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.visuals.window_fill = BG;
    style.visuals.panel_fill = BG;
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.hovered.bg_fill = SURFACE;
    style.visuals.widgets.active.bg_fill = BLUE;
    style.visuals.widgets.noninteractive.fg_stroke.color = MUTED;
    style.visuals.widgets.inactive.fg_stroke.color = TEXT;
    style.visuals.selection.bg_fill = BLUE;
    style.visuals.hyperlink_color = BLUE;
    style.visuals.extreme_bg_color = BG;
    style.spacing.item_spacing = egui::vec2(8.0, 0.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_global_style(style);
}

fn load_cache(path: &std::path::Path) -> Option<Vec<projects::Project>> {
    let start = std::time::Instant::now();
    let data = std::fs::read_to_string(path).ok()?;
    let projects: Vec<projects::Project> = serde_json::from_str(&data).ok()?;
    let elapsed = start.elapsed();
    eprintln!("[cache] loaded {} projects in {:.3}s", projects.len(), elapsed.as_secs_f64());
    Some(projects)
}

impl ProjectApp {
    fn filtered_projects(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                self.search_query.is_empty()
                    || p.name
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
            })
            .map(|(i, _)| i)
            .collect();

        indices.sort_by(|&a, &b| {
            self.projects[b]
                .last_modified
                .cmp(&self.projects[a].last_modified)
        });
        indices
    }

    fn open_in_vscode(&self, path: &std::path::Path) {
        let _ = std::process::Command::new("code").arg(path).spawn();
    }
}

impl eframe::App for ProjectApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if !self.logged_first_frame {
            self.logged_first_frame = true;
            eprintln!("[app] first frame at {:.3}s", self.app_start.elapsed().as_secs_f64());
        }

        let is_focused = ctx.input(|i| i.focused);
        if self.was_focused && !is_focused {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.was_focused = is_focused;

        setup_style(&ctx);

        // Check if background refresh completed
        if let Some(ref rx) = self.project_rx {
            if let Ok(fresh) = rx.try_recv() {
                self.projects = fresh;
                self.project_rx = None;
            }
        }

        if self.needs_reload || self.projects.is_empty() {
            self.projects.clear();
            self.project_rx = None;
            self.needs_reload = false;

            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/itaplyr".to_string());
            let base = match self.mode {
                Mode::Personal => PathBuf::from(&home).join("Projects"),
                Mode::Customers => PathBuf::from(&home).join("CustomerProjects"),
            };
            let cache_dir = PathBuf::from(&home).join(".cache/project-selector");
            let cache_file = match self.mode {
                Mode::Personal => cache_dir.join("personal.json"),
                Mode::Customers => cache_dir.join("customers.json"),
            };

            // Load cache instantly
            if let Some(cached) = load_cache(&cache_file) {
                self.projects = cached;
            }

            // Spawn background refresh
            let (tx, rx) = mpsc::channel();
            self.project_rx = Some(rx);
            std::thread::spawn(move || {
                let bg_start = std::time::Instant::now();
                eprintln!("[bg] refresh started");
                let fresh = projects::get_projects(&base);
                eprintln!("[bg] refresh done in {:.3}s ({} projects)", bg_start.elapsed().as_secs_f64(), fresh.len());
                if let Ok(data) = serde_json::to_string(&fresh) {
                    let _ = std::fs::create_dir_all(&cache_dir);
                    let _ = std::fs::write(&cache_file, &data);
                }
                let _ = tx.send(fresh);
            });
        }

        let filtered = self.filtered_projects();
        let flen = filtered.len();
        let show_create = flen == 0 && !self.search_query.is_empty();

        // Handle keyboard navigation before widgets process events
        let mut should_open = false;
        let mut should_close = false;
        let mut should_create = false;
        let mut should_delete = false;
        ctx.input(|i| {
            for e in &i.events {
                if self.confirm_delete.is_some() {
                    match e {
                        egui::Event::Key { key: egui::Key::ArrowLeft | egui::Key::ArrowRight, pressed: true, .. } => {
                            self.confirm_choice ^= 1;
                        }
                        egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                            if self.confirm_choice == 1 {
                                should_delete = true;
                            } else {
                                self.confirm_delete = None;
                            }
                        }
                        egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                            self.confirm_delete = None;
                        }
                        _ => {}
                    }
                } else {
                    match e {
                        egui::Event::Key { key: egui::Key::ArrowDown, pressed: true, .. } => {
                            if flen > 0 {
                                let pos = filtered.iter().position(|&x| x == self.selected).unwrap_or(0);
                                if pos + 1 < flen {
                                    self.selected = filtered[pos + 1];
                                }
                            }
                        }
                        egui::Event::Key { key: egui::Key::ArrowUp, pressed: true, .. } => {
                            if flen > 0 {
                                let pos = filtered.iter().position(|&x| x == self.selected).unwrap_or(0);
                                if pos > 0 {
                                    self.selected = filtered[pos - 1];
                                }
                            }
                        }
                        egui::Event::Key { key: egui::Key::Tab, pressed: true, .. } => {
                            self.mode = match self.mode {
                                Mode::Personal => Mode::Customers,
                                Mode::Customers => Mode::Personal,
                            };
                            self.needs_reload = true;
                            self.selected = 0;
                        }
                        egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                            should_close = true;
                        }
                        egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                            if show_create {
                                should_create = true;
                            } else if !filtered.is_empty() {
                                should_open = true;
                            }
                        }
                        egui::Event::Key { key: egui::Key::Delete, pressed: true, .. } => {
                            if !filtered.is_empty() && filtered.contains(&self.selected) {
                                self.confirm_delete = Some(self.selected);
                                self.confirm_choice = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        if self.selected >= self.projects.len() {
            self.selected = 0;
        }
        if !filtered.is_empty() && !filtered.contains(&self.selected) {
            self.selected = filtered[0];
        }

        // Remove consumed events so egui widgets don't also process them
        ctx.input_mut(|i| {
            if self.confirm_delete.is_some() {
                i.events.retain(|e| {
                    !matches!(e,
                        egui::Event::Key { key: egui::Key::ArrowDown | egui::Key::ArrowUp | egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::Tab, pressed: true, .. }
                    )
                });
            } else {
                i.events.retain(|e| {
                    !matches!(e,
                        egui::Event::Key { key: egui::Key::ArrowDown | egui::Key::ArrowUp | egui::Key::Tab, pressed: true, .. }
                    )
                });
            }
        });

        if should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if should_create {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/itaplyr".to_string());
            let base = match self.mode {
                Mode::Personal => PathBuf::from(&home).join("Projects"),
                Mode::Customers => PathBuf::from(&home).join("CustomerProjects"),
            };
            let new_path = base.join(&self.search_query);
            let _ = std::fs::create_dir_all(&new_path);
            self.open_in_vscode(&new_path);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if should_delete {
            if let Some(idx) = self.confirm_delete {
                if idx < self.projects.len() {
                    let path = self.projects[idx].path.clone();
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
            self.confirm_delete = None;
            self.projects.clear();
            self.needs_reload = true;
            return;
        }

        if should_open && self.selected < self.projects.len() {
            self.open_in_vscode(&self.projects[self.selected].path);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        egui::Frame::new().fill(BG).show(ui, |ui| {
            // Search bar at the top
            ui.add_space(6.0);
            let search_width = 420.0;
            let left = (ui.available_width() - search_width).max(0.0) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(left);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search projects...")
                        .desired_width(search_width)
                        .font(egui::TextStyle::Body)
                    .margin(egui::vec2(14.0, 16.0))
                    .frame(egui::Frame {
                        fill: SURFACE,
                        corner_radius: egui::CornerRadius::same(14),
                        inner_margin: egui::Margin::symmetric(10, 6),
                        ..Default::default()
                    }),
                );
                resp.request_focus();
            });

            // Mode tabs
            let tab_height = 28.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let total_w = ui.available_width();
                let tab_w = total_w / 2.0;
                for &mode in &[Mode::Personal, Mode::Customers] {
                    let label = match mode {
                        Mode::Personal => "Personal",
                        Mode::Customers => "Customers",
                    };
                    let active = self.mode == mode;
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(tab_w, tab_height),
                        egui::Sense::click(),
                    );

                    ui.painter().rect_filled(rect, 0.0, BG);

                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(12.0),
                        if active { TEXT } else { MUTED },
                    );

                    if active {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(rect.min.x, rect.max.y - 2.0),
                                egui::vec2(rect.width(), 2.0),
                            ),
                            0.0,
                            TEXT,
                        );
                    }

                    if resp.clicked() && self.mode != mode {
                        self.mode = mode;
                        self.needs_reload = true;
                        self.selected = 0;
                    }
                }
            });

            // Project list - fills remaining space
            ui.set_min_height(ui.available_height());
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    if show_create {
                        let height = 52.0;
                        let width = ui.available_width();
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(0.0), height), egui::Sense::click());

                        let border = egui::Rect::from_min_size(
                            rect.left_top(),
                            Vec2::new(3.0, height),
                        );
                        ui.painter().rect_filled(border, 0.0, BLUE);

                        let card_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x + 3.0, rect.min.y),
                            Vec2::new(width - 3.0, height),
                        );
                        ui.painter().rect_filled(card_rect, 6.0, SURFACE);

                        ui.painter().text(
                            egui::pos2(rect.min.x + 14.0, rect.min.y + 7.0),
                            egui::Align2::LEFT_TOP,
                            &format!("+  {}", self.search_query),
                            egui::FontId::proportional(14.0),
                            TEXT,
                        );

                        ui.painter().text(
                            egui::pos2(rect.min.x + 14.0, rect.min.y + 28.0),
                            egui::Align2::LEFT_TOP,
                            "Create new project",
                            egui::FontId::proportional(11.0),
                            MUTED,
                        );

                        resp.scroll_to_me(Some(egui::Align::Center));

                        if resp.clicked() {
                            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/itaplyr".to_string());
                            let base = match self.mode {
                                Mode::Personal => PathBuf::from(&home).join("Projects"),
                                Mode::Customers => PathBuf::from(&home).join("CustomerProjects"),
                            };
                            let new_path = base.join(&self.search_query);
                            let _ = std::fs::create_dir_all(&new_path);
                            self.open_in_vscode(&new_path);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            return;
                        }
                    }

                    for &idx in &filtered {
                        let project = &self.projects[idx];
                        let is_selected = idx == self.selected;
                        let _card_id = ui.id().with(format!("card_{}", idx));

                        let height = 52.0;
                        let width = ui.available_width();
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(0.0), height), egui::Sense::click());

                        let bg = if is_selected { SURFACE } else { egui::Color32::TRANSPARENT };

                        if is_selected {
                            let border = egui::Rect::from_min_size(
                                rect.left_top(),
                                Vec2::new(3.0, height),
                            );
                            ui.painter().rect_filled(border, 0.0, BLUE);
                        }

                        let card_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x + if is_selected { 3.0 } else { 0.0 }, rect.min.y),
                            Vec2::new(width - if is_selected { 3.0 } else { 0.0 }, height),
                        );
                        ui.painter().rect_filled(card_rect, 6.0, bg);

                        if resp.clicked() {
                            self.selected = idx;
                        }
                        if resp.double_clicked() {
                            self.open_in_vscode(&project.path);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            return;
                        }

                        if is_selected {
                            resp.scroll_to_me(Some(egui::Align::Center));
                        }

                        ui.painter().text(
                            egui::pos2(rect.min.x + 14.0, rect.min.y + 7.0),
                            egui::Align2::LEFT_TOP,
                            &project.name,
                            egui::FontId::proportional(14.0),
                            TEXT,
                        );

                        let (branch_label, meta_color) = if project.git_info.connected {
                            if project.git_info.files_changed > 0 {
                                (format!("{}  {} changed", project.git_info.branch, project.git_info.files_changed), YELLOW)
                            } else {
                                (project.git_info.branch.clone(), MUTED)
                            }
                        } else {
                            ("No git".to_string(), MUTED)
                        };

                        let meta = format!("{}   {}", time_ago(project.last_modified), branch_label);

                        ui.painter().text(
                            egui::pos2(rect.min.x + 14.0, rect.min.y + 28.0),
                            egui::Align2::LEFT_TOP,
                            &meta,
                            egui::FontId::proportional(11.0),
                            meta_color,
                        );
                    }
                });
        });

        if let Some(idx) = self.confirm_delete {
            let name = if idx < self.projects.len() {
                self.projects[idx].name.clone()
            } else {
                String::new()
            };
            let screen = ctx.viewport_rect();

            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));

            let dlg = egui::Rect::from_center_size(screen.center(), egui::vec2(360.0, 120.0));
            ui.painter().rect_filled(dlg, 8.0, SURFACE);

            ui.painter().text(
                egui::pos2(dlg.center().x, dlg.min.y + 30.0),
                egui::Align2::CENTER_CENTER,
                &format!("Delete \"{}\"?", name),
                egui::FontId::proportional(14.0),
                TEXT,
            );

            for i in 0..2 {
                let label = if i == 0 { "Cancel" } else { "Delete" };
                let selected = self.confirm_choice == i;
                let x = dlg.center().x + (i as f32 - 0.5) * 96.0 - 40.0;
                let btn = egui::Rect::from_min_size(
                    egui::pos2(x, dlg.max.y - 36.0),
                    egui::vec2(80.0, 28.0),
                );

                if selected {
                    ui.painter().rect_filled(btn, 4.0, BLUE);
                } else {
                    ui.painter().rect_stroke(btn, 4.0, egui::Stroke::new(1.0, MUTED), egui::StrokeKind::Inside);
                }

                ui.painter().text(
                    btn.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    if selected { BG } else { TEXT },
                );
            }
        }
    }
}

