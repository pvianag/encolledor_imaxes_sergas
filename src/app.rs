use crate::about;
use crate::config::AppConfig;
use crate::i18n::{self, Lang, Strings};
use crate::zip_ops::{
    analyze_zip, format_bytes, output_path_for, shrink_many, ProgressFn, ShrinkResult, ZipAnalysis,
};
use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, Frame, Margin, RichText, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

const SERGAS_LOGO_PNG: &[u8] = include_bytes!("../assets/sergas_logo.png");
const RADIOGRAPHY_PNG: &[u8] = include_bytes!("../assets/radiography.png");
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/app_icon.png");

#[derive(Debug, Clone)]
struct QueueItem {
    path: PathBuf,
    analysis: Option<ZipAnalysis>,
    analyzing: bool,
}

enum WorkerMsg {
    Analyzed(PathBuf, ZipAnalysis),
    Progress {
        file_index: usize,
        total_files: usize,
        frac: f32,
        name: String,
    },
    Finished(Result<Vec<ShrinkResult>, String>),
}

enum AppPhase {
    Idle,
    ConfirmOverwrite {
        existing: Vec<PathBuf>,
    },
    Working,
    AskDelete {
        results: Vec<ShrinkResult>,
        remember: bool,
    },
    Done {
        results: Vec<ShrinkResult>,
        deleted: bool,
    },
}

pub struct ShrinkApp {
    config: AppConfig,
    lang: Lang,
    queue: Vec<QueueItem>,
    phase: AppPhase,
    status: String,
    progress: f32,
    progress_label: String,
    last_error: Option<String>,
    tx: Sender<WorkerMsg>,
    rx: Receiver<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    sergas_logo: Option<TextureHandle>,
    radiography: Option<TextureHandle>,
    app_icon: Option<TextureHandle>,
    show_about: bool,
    /// Grow the OS window when content/phase needs more vertical space.
    window_fitted: bool,
    last_phase_key: u8,
}

impl ShrinkApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        let lang = Lang::from_code(&config.language);
        let (tx, rx) = mpsc::channel();

        let sergas_logo = load_texture(&cc.egui_ctx, "sergas_logo", SERGAS_LOGO_PNG, false);
        let radiography = load_texture(&cc.egui_ctx, "radiography", RADIOGRAPHY_PNG, true);
        let app_icon = load_texture(&cc.egui_ctx, "app_icon", APP_ICON_PNG, false);

        Self {
            config,
            lang,
            queue: Vec::new(),
            phase: AppPhase::Idle,
            status: String::new(),
            progress: 0.0,
            progress_label: String::new(),
            last_error: None,
            tx,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            sergas_logo,
            radiography,
            app_icon,
            show_about: false,
            window_fitted: false,
            last_phase_key: 0,
        }
    }

    fn phase_key(&self) -> u8 {
        match self.phase {
            AppPhase::Idle => 0,
            AppPhase::ConfirmOverwrite { .. } => 1,
            AppPhase::Working => 2,
            AppPhase::AskDelete { .. } => 3,
            AppPhase::Done { .. } => 4,
        }
    }

    fn t(&self) -> Strings {
        i18n::strings(self.lang)
    }

    fn persist(&mut self) {
        self.config.language = self.lang.code().to_string();
        let _ = self.config.save();
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let is_zip = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("zip"))
                .unwrap_or(false);
            if !is_zip {
                continue;
            }
            if self.queue.iter().any(|q| q.path == path) {
                continue;
            }
            if let Some(parent) = path.parent() {
                self.config.last_input_dir = parent.display().to_string();
            }
            self.queue.push(QueueItem {
                path: path.clone(),
                analysis: None,
                analyzing: true,
            });
            let tx = self.tx.clone();
            thread::spawn(move || {
                let analysis = analyze_zip(&path);
                let _ = tx.send(WorkerMsg::Analyzed(path, analysis));
            });
        }
        self.persist();
        self.last_error = None;
        if matches!(self.phase, AppPhase::Done { .. }) {
            self.phase = AppPhase::Idle;
        }
    }

    fn totals(&self) -> (u64, u64, u64, usize, usize, bool) {
        let mut input = 0u64;
        let mut est_out = 0u64;
        let mut keep = 0usize;
        let mut drop = 0usize;
        let mut all_ready = !self.queue.is_empty();
        for q in &self.queue {
            if q.analyzing || q.analysis.is_none() {
                all_ready = false;
                continue;
            }
            if let Some(a) = &q.analysis {
                input += a.input_size;
                est_out += a.estimated_output;
                keep += a.keep_count;
                drop += a.drop_count;
                if !a.is_processable() {
                    all_ready = false;
                }
            }
        }
        let saved = input.saturating_sub(est_out);
        (input, est_out, saved, keep, drop, all_ready)
    }

    fn existing_outputs(&self) -> Vec<PathBuf> {
        self.queue
            .iter()
            .filter_map(|q| q.analysis.as_ref())
            .filter(|a| a.is_processable())
            .map(|a| output_path_for(&a.path))
            .filter(|p| p.exists())
            .collect()
    }

    fn request_shrink(&mut self) {
        let existing = self.existing_outputs();
        if !existing.is_empty() {
            self.phase = AppPhase::ConfirmOverwrite { existing };
            return;
        }
        self.start_shrink();
    }

    fn start_shrink(&mut self) {
        let t = self.t();
        let analyses: Vec<ZipAnalysis> = self
            .queue
            .iter()
            .filter_map(|q| q.analysis.clone())
            .filter(|a| a.is_processable())
            .collect();
        if analyses.is_empty() {
            self.last_error = Some(t.no_dicom.to_string());
            self.phase = AppPhase::Idle;
            return;
        }

        self.cancel.store(false, Ordering::Relaxed);
        self.phase = AppPhase::Working;
        self.progress = 0.0;
        self.progress_label.clear();
        self.status = t.processing.to_string();
        self.last_error = None;

        let tx = self.tx.clone();
        let cancel = self.cancel.clone();
        thread::spawn(move || {
            let progress: ProgressFn = Box::new({
                let tx = tx.clone();
                move |file_index, total_files, frac, name| {
                    let _ = tx.send(WorkerMsg::Progress {
                        file_index,
                        total_files,
                        frac,
                        name: name.to_string(),
                    });
                }
            });
            let result = shrink_many(&analyses, cancel, progress).map_err(|e| e.to_string());
            let _ = tx.send(WorkerMsg::Finished(result));
        });
    }

    fn poll_worker(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                WorkerMsg::Analyzed(path, analysis) => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.path == path) {
                        item.analyzing = false;
                        item.analysis = Some(analysis);
                    }
                }
                WorkerMsg::Progress {
                    file_index,
                    total_files,
                    frac,
                    name,
                } => {
                    let t = self.t();
                    let overall = if total_files == 0 {
                        frac
                    } else {
                        (file_index as f32 + frac) / total_files as f32
                    };
                    self.progress = overall.clamp(0.0, 1.0);
                    self.progress_label = format!(
                        "{} {}/{} — {}",
                        t.queue_progress,
                        file_index + 1,
                        total_files,
                        name
                    );
                    ctx.request_repaint();
                }
                WorkerMsg::Finished(result) => match result {
                    Ok(results) => {
                        let t = self.t();
                        self.progress = 1.0;
                        self.status = t.done.to_string();
                        if self.config.ask_delete_original {
                            self.phase = AppPhase::AskDelete {
                                results,
                                remember: false,
                            };
                        } else {
                            if self.config.default_delete_original {
                                for r in &results {
                                    let _ = std::fs::remove_file(&r.input);
                                }
                            }
                            let deleted = self.config.default_delete_original;
                            self.phase = AppPhase::Done { results, deleted };
                        }
                    }
                    Err(err) => {
                        if err == "cancelled" {
                            self.status = self.t().cancel.to_string();
                            self.phase = AppPhase::Idle;
                        } else {
                            self.last_error = Some(err);
                            self.phase = AppPhase::Idle;
                        }
                    }
                },
            }
        }
    }

    fn apply_delete_choice(&mut self, delete: bool, remember: bool) {
        let results = match &self.phase {
            AppPhase::AskDelete { results, .. } => results.clone(),
            _ => return,
        };
        if remember {
            self.config.remember_delete_choice = true;
            self.config.ask_delete_original = false;
            self.config.default_delete_original = delete;
            self.persist();
        }
        if delete {
            for r in &results {
                let _ = std::fs::remove_file(&r.input);
            }
        }
        self.phase = AppPhase::Done {
            results,
            deleted: delete,
        };
    }
}

impl eframe::App for ShrinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker(ctx);
        let t = self.t();
        if self.status.is_empty() {
            self.status = t.status_idle.to_string();
        }

        // Theme: SERGAS navy / cool clinical slate
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(14.0, 8.0);
        ctx.set_style(style);

        let bg = Color32::from_rgb(241, 245, 249);
        let panel = Color32::from_rgb(255, 255, 255);
        let accent = Color32::from_rgb(0, 32, 91); // SERGAS navy
        let accent_soft = Color32::from_rgb(220, 230, 242);
        let text = Color32::from_rgb(30, 41, 51);
        let muted = Color32::from_rgb(100, 116, 139);
        let ok = Color32::from_rgb(21, 128, 61);
        let danger = Color32::from_rgb(185, 28, 28);

        // Watermark on the background layer only (never covers interactive widgets)
        if let Some(tex) = &self.radiography {
            let screen = ctx.screen_rect();
            let side = (screen.width().min(screen.height()) * 0.38).clamp(140.0, 240.0);
            let size = Vec2::splat(side);
            let pos = egui::pos2(
                screen.right() - size.x - 20.0,
                screen.bottom() - size.y - 20.0,
            );
            let rect = egui::Rect::from_min_size(pos, size);
            ctx.layer_painter(egui::LayerId::background()).image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_white_alpha(22),
            );
        }

        // Reserve a bottom action strip so prompts/buttons never overlap the diagram
        let (input, est_out, saved, keep, dropc, all_ready) = self.totals();
        let analyzing_any = self.queue.iter().any(|q| q.analyzing);
        let footer_h = match &self.phase {
            AppPhase::ConfirmOverwrite { existing } => {
                108.0 + (existing.len().min(4) as f32) * 18.0
            }
            AppPhase::AskDelete { .. } => 110.0,
            AppPhase::Working => 96.0,
            AppPhase::Done { .. } => 72.0,
            AppPhase::Idle => 64.0,
        };

        egui::TopBottomPanel::bottom("action_footer")
            .exact_height(footer_h)
            .frame(
                Frame::new()
                    .fill(panel)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(226, 232, 240)))
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                self.draw_footer_actions(ui, &t, accent, muted, danger, ok, text, all_ready, analyzing_any);
            });

        let mut content_size = Vec2::ZERO;
        egui::CentralPanel::default()
            .frame(Frame::new().fill(bg).inner_margin(Margin::same(18)))
            .show(ctx, |ui| {
                let scroll = egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt("main_scroll")
                    .show(ui, |ui| {
                        ui.set_min_width((ui.available_width() - 4.0).max(640.0));
                ui.horizontal(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(tex) = &self.app_icon {
                            ui.add(
                                egui::Image::new(tex)
                                    .fit_to_exact_size(Vec2::splat(48.0))
                                    .corner_radius(CornerRadius::same(10))
                                    .sense(Sense::hover()),
                            );
                            ui.add_space(8.0);
                        }
                        if let Some(tex) = &self.sergas_logo {
                            let logo_h = 48.0_f32;
                            let aspect = tex.size_vec2().x / tex.size_vec2().y.max(1.0);
                            let logo_w = logo_h * aspect;
                            ui.add(
                                egui::Image::new(tex)
                                    .fit_to_exact_size(Vec2::new(logo_w, logo_h))
                                    .sense(Sense::hover()),
                            );
                            ui.add_space(10.0);
                            ui.vertical(|ui| {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("ZIP Shrinker")
                                        .color(accent)
                                        .size(22.0)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        about::GIT_TAG,
                                        about::GIT_COMMIT_SHORT
                                    ))
                                    .color(muted)
                                    .size(12.0),
                                );
                            });
                        } else {
                            ui.heading(
                                RichText::new(t.app_title)
                                    .color(accent)
                                    .size(26.0)
                                    .strong(),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // About first in RTL ⇒ pinned to the far right (always visible)
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t.about).color(Color32::WHITE).strong(),
                                )
                                .fill(accent)
                                .corner_radius(CornerRadius::same(6))
                                .min_size(Vec2::new(88.0, 28.0)),
                            )
                            .on_hover_text(format!(
                                "{} {} ({})",
                                t.about_version,
                                about::VERSION,
                                about::GIT_TAG
                            ))
                            .clicked()
                        {
                            self.show_about = true;
                        }
                        ui.add_space(8.0);
                        for lang in Lang::all().iter().rev() {
                            let selected = self.lang == *lang;
                            if flag_button(ui, *lang, selected).clicked() {
                                self.lang = *lang;
                                self.persist();
                                self.status =
                                    i18n::strings(self.lang).status_idle.to_string();
                            }
                        }
                        ui.label(RichText::new(t.language).color(muted));
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t.disclaimer).size(12.0).color(muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .link(
                                RichText::new(format!(
                                    "{} {}",
                                    t.about_version,
                                    about::GIT_DESCRIBE
                                ))
                                .size(12.0)
                                .color(accent),
                            )
                            .on_hover_text(t.about)
                            .clicked()
                        {
                            self.show_about = true;
                        }
                    });
                });
                ui.add_space(8.0);

                // Drop zone
                let drop_frame = Frame::new()
                    .fill(accent_soft)
                    .stroke(Stroke::new(1.5_f32, accent))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::same(20));

                drop_frame.show(ui, |ui| {
                    ui.set_min_height(96.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(t.drop_hint).size(16.0).color(text));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !matches!(self.phase, AppPhase::Working),
                                    egui::Button::new(RichText::new(t.choose_files).color(Color32::WHITE))
                                        .fill(accent),
                                )
                                .clicked()
                            {
                                let mut dialog = rfd::FileDialog::new()
                                    .add_filter("ZIP", &["zip"]);
                                if !self.config.last_input_dir.is_empty() {
                                    dialog = dialog.set_directory(&self.config.last_input_dir);
                                }
                                if let Some(files) = dialog.pick_files() {
                                    self.add_paths(files);
                                }
                            }
                            if ui
                                .add_enabled(
                                    !self.queue.is_empty()
                                        && !matches!(self.phase, AppPhase::Working),
                                    egui::Button::new(t.clear_list),
                                )
                                .clicked()
                            {
                                self.queue.clear();
                                self.phase = AppPhase::Idle;
                                self.status = t.status_idle.to_string();
                                self.last_error = None;
                            }
                        });
                        ui.add_space(8.0);
                    });

                });

                let dropped: Vec<PathBuf> = ctx.input(|i| {
                    i.raw
                        .dropped_files
                        .iter()
                        .filter_map(|f| f.path.clone())
                        .collect()
                });
                if !dropped.is_empty() && !matches!(self.phase, AppPhase::Working) {
                    self.add_paths(dropped);
                    ctx.input_mut(|i| i.raw.dropped_files.clear());
                }

                ui.add_space(12.0);
                ui.label(RichText::new(t.output_suffix_note).size(12.5).color(muted));
                ui.add_space(6.0);

                // Queue list
                Frame::new()
                    .fill(panel)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(RichText::new(t.files).strong().color(text));
                        ui.add_space(4.0);
                        if self.queue.is_empty() {
                            ui.label(RichText::new("—").color(muted));
                        } else {
                            egui::ScrollArea::vertical()
                                .max_height(140.0)
                                .show(ui, |ui| {
                                    for item in &self.queue {
                                        let name = item
                                            .path
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("?");
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(name).color(text));
                                            if item.analyzing {
                                                ui.label(
                                                    RichText::new(t.analyzing).color(accent).italics(),
                                                );
                                            } else if let Some(a) = &item.analysis {
                                                if a.is_processable() {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} → {}",
                                                            format_bytes(a.input_size),
                                                            format_bytes(a.estimated_output)
                                                        ))
                                                        .color(ok),
                                                    );
                                                } else {
                                                    let msg = if a.error.as_deref() == Some("no_dicom")
                                                    {
                                                        t.no_dicom
                                                    } else {
                                                        a.error.as_deref().unwrap_or(t.invalid_zip)
                                                    };
                                                    ui.label(RichText::new(msg).color(danger));
                                                }
                                            }
                                        });
                                    }
                                });
                        }
                    });

                ui.add_space(12.0);

                // Analysis / results card (actions live in the bottom panel)
                let show_estimate_card = !matches!(self.phase, AppPhase::Done { .. });

                if show_estimate_card {
                    Frame::new()
                        .fill(panel)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::same(14))
                        .show(ui, |ui| {
                            size_diagram(
                                ui,
                                SizeDiagramLabels {
                                    input_label: t.input_size,
                                    output_label: t.est_output,
                                    saved_label: t.est_saved,
                                },
                                input,
                                est_out,
                                saved,
                                accent,
                                ok,
                                muted,
                                text,
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{}: {keep}", t.keep_count))
                                        .color(muted),
                                );
                                ui.separator();
                                ui.label(
                                    RichText::new(format!("{}: {dropc}", t.drop_count))
                                        .color(muted),
                                );
                                if analyzing_any {
                                    ui.separator();
                                    ui.label(RichText::new(t.analyzing).color(accent));
                                }
                            });
                        });
                } else if let AppPhase::Done { results, deleted } = &self.phase {
                    let results = results.clone();
                    let deleted = *deleted;
                    let total_in: u64 = results.iter().map(|r| r.input_size).sum();
                    let total_out: u64 = results.iter().map(|r| r.output_size).sum();
                    let saved_n = total_in.saturating_sub(total_out);
                    ui.label(RichText::new(t.done).color(ok).strong().size(16.0));
                    ui.add_space(6.0);
                    Frame::new()
                        .fill(panel)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::same(14))
                        .show(ui, |ui| {
                            size_diagram(
                                ui,
                                SizeDiagramLabels {
                                    input_label: t.input_size,
                                    output_label: t.actual_output,
                                    saved_label: t.actual_saved,
                                },
                                total_in,
                                total_out,
                                saved_n,
                                accent,
                                ok,
                                muted,
                                text,
                            );
                        });
                    if deleted {
                        ui.add_space(6.0);
                        ui.label(RichText::new(t.delete_yes).color(muted).small());
                    }
                }

                if let Some(err) = &self.last_error {
                    ui.add_space(10.0);
                    ui.colored_label(danger, format!("{}: {err}", t.error));
                }

                ui.add_space(12.0);
                    });
                content_size = scroll.content_size;
            });

        // Re-fit when the footer phase changes (overwrite / delete prompts need height)
        let phase_key = self.phase_key();
        if phase_key != self.last_phase_key {
            self.last_phase_key = phase_key;
            self.window_fitted = false;
        }

        if !self.window_fitted {
            let margin = Vec2::new(48.0, footer_h + 48.0);
            let needed = content_size + margin;
            let screen = ctx.screen_rect().size();
            let target = Vec2::new(
                needed.x.clamp(720.0, 1100.0).max(screen.x),
                needed.y.clamp(720.0, 1100.0).max(screen.y),
            );
            if target.x > screen.x + 8.0 || target.y > screen.y + 8.0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
                ctx.request_repaint();
            } else {
                self.window_fitted = true;
            }
        }

        if self.show_about {
            self.about_overlay(ctx, accent, accent_soft, panel, text, muted, ok);
        }

        if matches!(self.phase, AppPhase::Working) || self.queue.iter().any(|q| q.analyzing) {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

impl ShrinkApp {
    fn draw_footer_actions(
        &mut self,
        ui: &mut egui::Ui,
        t: &Strings,
        accent: Color32,
        muted: Color32,
        danger: Color32,
        ok: Color32,
        text: Color32,
        all_ready: bool,
        analyzing_any: bool,
    ) {
        match &self.phase {
            AppPhase::ConfirmOverwrite { existing } => {
                let existing = existing.clone();
                ui.colored_label(
                    Color32::from_rgb(180, 83, 9),
                    RichText::new(t.overwrite_warn).strong(),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(72.0)
                    .show(ui, |ui| {
                        for p in &existing {
                            let name = p
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("?");
                            ui.label(RichText::new(format!("• {name}")).color(text));
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t.overwrite_yes).color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(180, 83, 9)),
                        )
                        .clicked()
                    {
                        self.start_shrink();
                    }
                    if ui.button(t.overwrite_no).clicked() {
                        self.phase = AppPhase::Idle;
                    }
                });
            }
            AppPhase::Working => {
                ui.add(
                    egui::ProgressBar::new(self.progress)
                        .show_percentage()
                        .animate(true)
                        .desired_width(ui.available_width()),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(&self.progress_label).color(muted));
                ui.add_space(4.0);
                if ui.button(t.cancel).clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                }
            }
            AppPhase::AskDelete { remember, .. } => {
                let mut remember_flag = *remember;
                ui.label(RichText::new(t.ask_delete).size(15.0).color(text));
                ui.checkbox(&mut remember_flag, t.remember_delete);
                if let AppPhase::AskDelete { remember, .. } = &mut self.phase {
                    *remember = remember_flag;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t.delete_yes).color(Color32::WHITE),
                            )
                            .fill(danger),
                        )
                        .clicked()
                    {
                        let rem = remember_flag;
                        self.apply_delete_choice(true, rem);
                    }
                    if ui.button(t.delete_no).clicked() {
                        let rem = remember_flag;
                        self.apply_delete_choice(false, rem);
                    }
                });
            }
            AppPhase::Done { results, .. } => {
                let folder = results
                    .first()
                    .and_then(|r| r.output.parent().map(|p| p.to_path_buf()));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t.done).color(ok).strong());
                    if let Some(parent) = folder {
                        if ui.button(t.open_folder).clicked() {
                            open_folder(&parent);
                        }
                    }
                });
            }
            AppPhase::Idle => {
                let can_run = all_ready
                    && !analyzing_any
                    && self.queue.iter().any(|q| {
                        q.analysis
                            .as_ref()
                            .map(|a| a.is_processable())
                            .unwrap_or(false)
                    });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            can_run,
                            egui::Button::new(
                                RichText::new(t.shrink).color(Color32::WHITE).size(16.0),
                            )
                            .fill(accent)
                            .min_size(Vec2::new(140.0, 36.0)),
                        )
                        .clicked()
                    {
                        self.request_shrink();
                    }
                    ui.label(RichText::new(&self.status).color(muted));
                });
            }
        }
    }

    fn about_overlay(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        accent_soft: Color32,
        panel: Color32,
        text: Color32,
        muted: Color32,
        ok: Color32,
    ) {
        let t = self.t();
        let mut open = self.show_about;
        let mut close = false;

        egui::Area::new(egui::Id::new("about_dim"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let response = ui.allocate_response(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::ZERO,
                    Color32::from_black_alpha(140),
                );
                if response.clicked() {
                    close = true;
                }
            });

        egui::Window::new(t.about)
            .id(egui::Id::new("about_window"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .frame(
                Frame::new()
                    .fill(panel)
                    .stroke(Stroke::new(1.0_f32, accent))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::same(18))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 28,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    }),
            )
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);

                ui.horizontal(|ui| {
                    if let Some(tex) = &self.app_icon {
                        ui.add(
                            egui::Image::new(tex)
                                .fit_to_exact_size(Vec2::splat(56.0))
                                .corner_radius(CornerRadius::same(12)),
                        );
                        ui.add_space(10.0);
                    }
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(about::APP_NAME)
                                .size(20.0)
                                .strong()
                                .color(accent),
                        );
                        ui.label(RichText::new(t.disclaimer).size(12.0).color(muted));
                    });
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                about_row(ui, t.about_version, about::VERSION, text, muted);
                about_row(ui, t.about_tag, about::GIT_TAG, text, muted);
                about_row(ui, t.about_release, about::GIT_DESCRIBE, text, muted);
                about_row(
                    ui,
                    t.about_commit,
                    &format!("{} ({})", about::GIT_COMMIT_SHORT, about::GIT_COMMIT),
                    text,
                    muted,
                );

                ui.add_space(6.0);
                ui.label(RichText::new(t.about_project).small().color(muted));
                if ui
                    .link(RichText::new(about::GITHUB_URL).color(Color32::from_rgb(37, 99, 235)))
                    .clicked()
                {
                    about::open_url(about::GITHUB_URL);
                }

                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t.about_open_github).color(Color32::WHITE),
                            )
                            .fill(accent),
                        )
                        .clicked()
                    {
                        about::open_url(about::GITHUB_URL);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new(t.about_open_release).color(accent))
                                .fill(accent_soft),
                        )
                        .clicked()
                    {
                        about::open_url(&about::release_url());
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new(t.about_open_commit).color(accent))
                                .fill(accent_soft),
                        )
                        .clicked()
                    {
                        about::open_url(&about::commit_url());
                    }
                });

                ui.add_space(10.0);
                ui.label(RichText::new(t.about_license).small().color(ok));
                ui.add_space(8.0);
                if ui.button(t.about_close).clicked() {
                    close = true;
                }
            });

        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.show_about = open;
    }
}

fn about_row(ui: &mut egui::Ui, label: &str, value: &str, text: Color32, muted: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label}:")).color(muted));
        ui.label(RichText::new(value).color(text).strong());
    });
}

struct SizeDiagramLabels {
    input_label: &'static str,
    output_label: &'static str,
    saved_label: &'static str,
}

/// Comparative size chart: savings ring + labeled progress bars (no overlapping painters).
fn size_diagram(
    ui: &mut egui::Ui,
    labels: SizeDiagramLabels,
    input: u64,
    output: u64,
    saved: u64,
    accent: Color32,
    ok: Color32,
    muted: Color32,
    text: Color32,
) {
    let pct = if input > 0 {
        (saved as f32 / input as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let out_ratio = if input > 0 {
        (output as f32 / input as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let track = Color32::from_rgb(226, 232, 240);

    ui.horizontal(|ui| {
        // Fixed-width donut column
        ui.allocate_ui_with_layout(
            Vec2::new(110.0, 110.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                let donut_size = Vec2::splat(100.0);
                let (donut_rect, _) = ui.allocate_exact_size(donut_size, Sense::hover());
                let painter = ui.painter_at(donut_rect);
                let center = donut_rect.center();
                let radius = 38.0_f32;
                let stroke_w = 10.0_f32;

                painter.circle_stroke(center, radius, Stroke::new(stroke_w, track));
                if pct > 0.001 {
                    let segments = 72;
                    let start = -std::f32::consts::FRAC_PI_2;
                    let sweep = pct * std::f32::consts::TAU;
                    let mut points = Vec::with_capacity(segments + 1);
                    for i in 0..=segments {
                        let a = start + sweep * (i as f32 / segments as f32);
                        points.push(egui::pos2(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ));
                    }
                    painter.add(egui::Shape::line(points, Stroke::new(stroke_w, ok)));
                }
                painter.text(
                    center - Vec2::new(0.0, 6.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}%", pct * 100.0),
                    egui::FontId::proportional(20.0),
                    ok,
                );
                painter.text(
                    center + Vec2::new(0.0, 14.0),
                    egui::Align2::CENTER_CENTER,
                    labels.saved_label,
                    egui::FontId::proportional(10.0),
                    muted,
                );
            },
        );

        ui.add_space(16.0);

        // Bars column — use native ProgressBar to avoid custom-paint overlap
        ui.vertical(|ui| {
            let bar_w = (ui.available_width() - 8.0).max(200.0);
            ui.set_max_width(bar_w);

            metric_bar(
                ui,
                labels.input_label,
                &format_bytes(input),
                if input > 0 { 1.0 } else { 0.0 },
                Color32::from_rgb(100, 116, 139),
                track,
                text,
                muted,
            );
            ui.add_space(12.0);
            metric_bar(
                ui,
                labels.output_label,
                &format_bytes(output),
                out_ratio,
                accent,
                track,
                text,
                muted,
            );
            ui.add_space(12.0);
            metric_bar(
                ui,
                labels.saved_label,
                &format!("{} ({:.0}%)", format_bytes(saved), pct * 100.0),
                pct,
                ok,
                track,
                text,
                muted,
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                swatch(ui, accent);
                ui.label(RichText::new(labels.output_label).small().color(muted));
                ui.add_space(14.0);
                swatch(ui, ok);
                ui.label(RichText::new(labels.saved_label).small().color(muted));
            });
        });
    });
}

fn metric_bar(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    ratio: f32,
    fill: Color32,
    track: Color32,
    text: Color32,
    muted: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).small().color(muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).strong().color(text));
        });
    });
    ui.add_space(3.0);
    ui.add(
        egui::ProgressBar::new(ratio.clamp(0.0, 1.0))
            .desired_height(16.0)
            .desired_width(ui.available_width())
            .fill(fill)
            .corner_radius(CornerRadius::same(5)),
    );
    // Subtle track hint behind empty bars
    let _ = track;
}

fn swatch(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), color);
}

fn load_texture(
    ctx: &egui::Context,
    name: &str,
    bytes: &[u8],
    watermark: bool,
) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut pixels = img.into_raw();

    if watermark {
        // Keep bright strokes; make dark background fully transparent for soft overlay.
        for px in pixels.chunks_exact_mut(4) {
            let lum = (usize::from(px[0]) + usize::from(px[1]) + usize::from(px[2])) / 3;
            if lum < 40 {
                px[3] = 0;
            } else {
                px[0] = 0;
                px[1] = 32;
                px[2] = 91;
                px[3] = (lum as u8).saturating_sub(20);
            }
        }
    }

    let color = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture(name, color, TextureOptions::LINEAR))
}

fn flag_button(ui: &mut egui::Ui, lang: Lang, selected: bool) -> egui::Response {
    let size = Vec2::new(34.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let response = response.on_hover_text(lang.label());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let inner = rect.shrink(2.0);
        paint_flag(painter, inner, lang);

        let border = if selected {
            Stroke::new(2.0_f32, Color32::from_rgb(0, 32, 91))
        } else if response.hovered() {
            Stroke::new(1.5_f32, Color32::from_rgb(100, 116, 139))
        } else {
            Stroke::new(1.0_f32, Color32::from_rgb(148, 163, 184))
        };
        painter.rect_stroke(inner, CornerRadius::same(3), border, egui::StrokeKind::Outside);
    }

    response
}

fn paint_flag(painter: &egui::Painter, rect: egui::Rect, lang: Lang) {
    match lang {
        Lang::Gl => {
            // Galicia: white field + light-blue diagonal band (top-left → bottom-right)
            painter.rect_filled(rect, CornerRadius::same(2), Color32::WHITE);
            let light_blue = Color32::from_rgb(0, 152, 213);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(rect.left(), rect.top() + rect.height() * 0.18),
                    egui::pos2(rect.left(), rect.top() + rect.height() * 0.48),
                    egui::pos2(rect.right(), rect.bottom() - rect.height() * 0.18),
                    egui::pos2(rect.right(), rect.bottom() - rect.height() * 0.48),
                ],
                light_blue,
                Stroke::NONE,
            ));
        }
        Lang::Es => {
            let h = rect.height() / 3.0;
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), h)),
                CornerRadius::ZERO,
                Color32::from_rgb(198, 11, 30),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.top() + h),
                    Vec2::new(rect.width(), h),
                ),
                CornerRadius::ZERO,
                Color32::from_rgb(255, 196, 0),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.top() + 2.0 * h),
                    Vec2::new(rect.width(), rect.height() - 2.0 * h),
                ),
                CornerRadius::ZERO,
                Color32::from_rgb(198, 11, 30),
            );
        }
        Lang::En => {
            painter.rect_filled(rect, CornerRadius::same(2), Color32::from_rgb(1, 33, 105));
            // St. George cross
            let t = (rect.height() * 0.18).max(2.0);
            painter.rect_filled(
                egui::Rect::from_center_size(rect.center(), Vec2::new(rect.width(), t)),
                CornerRadius::ZERO,
                Color32::WHITE,
            );
            painter.rect_filled(
                egui::Rect::from_center_size(rect.center(), Vec2::new(t, rect.height())),
                CornerRadius::ZERO,
                Color32::WHITE,
            );
            let t2 = t * 0.45;
            painter.rect_filled(
                egui::Rect::from_center_size(rect.center(), Vec2::new(rect.width(), t2)),
                CornerRadius::ZERO,
                Color32::from_rgb(200, 16, 46),
            );
            painter.rect_filled(
                egui::Rect::from_center_size(rect.center(), Vec2::new(t2, rect.height())),
                CornerRadius::ZERO,
                Color32::from_rgb(200, 16, 46),
            );
        }
        Lang::Fr => {
            let w = rect.width() / 3.0;
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
                CornerRadius::ZERO,
                Color32::from_rgb(0, 85, 164),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + w, rect.top()),
                    Vec2::new(w, rect.height()),
                ),
                CornerRadius::ZERO,
                Color32::WHITE,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 2.0 * w, rect.top()),
                    Vec2::new(rect.width() - 2.0 * w, rect.height()),
                ),
                CornerRadius::ZERO,
                Color32::from_rgb(239, 65, 53),
            );
        }
        Lang::Pt => {
            let green_w = rect.width() * 0.4;
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(green_w, rect.height())),
                CornerRadius::ZERO,
                Color32::from_rgb(0, 102, 0),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + green_w, rect.top()),
                    Vec2::new(rect.width() - green_w, rect.height()),
                ),
                CornerRadius::ZERO,
                Color32::from_rgb(255, 0, 0),
            );
            painter.circle_filled(
                egui::pos2(rect.left() + green_w, rect.center().y),
                rect.height() * 0.22,
                Color32::from_rgb(255, 204, 0),
            );
        }
    }
}

fn open_folder(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}
