use std::path::PathBuf;

use eframe::egui::{self, Vec2};
use crate::projects;

const BG: egui::Color32 = egui::Color32::from_rgb(0x1f, 0x1f, 0x21);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(0x24, 0x24, 0x26);
const TEXT: egui::Color32 = egui::Color32::from_rgb(0x92, 0x90, 0x92);
const SUBTEXT: egui::Color32 = egui::Color32::from_rgb(0x92, 0x90, 0x92);
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

        let is_focused = ctx.input(|i| i.focused);
        if self.was_focused && !is_focused {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.was_focused = is_focused;

        setup_style(&ctx);

        if self.needs_reload || self.projects.is_empty() {
            self.projects.clear();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/itaplyr".to_string());
            let base = match self.mode {
                Mode::Personal => PathBuf::from(&home).join("Projects"),
                Mode::Customers => PathBuf::from(&home).join("CustomerProjects"),
            };
            self.projects = projects::get_projects(&base);
            self.needs_reload = false;
        }

        let filtered = self.filtered_projects();
        let flen = filtered.len();

        // Handle keyboard navigation before widgets process events
        let mut should_open = false;
        let mut should_close = false;
        ctx.input(|i| {
            for e in &i.events {
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
                        if !filtered.is_empty() {
                            should_open = true;
                        }
                    }
                    _ => {}
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
            i.events.retain(|e| {
                !matches!(e,
                    egui::Event::Key { key: egui::Key::ArrowDown | egui::Key::ArrowUp | egui::Key::Tab, pressed: true, .. }
                )
            });
        });

        if should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
    }
}
