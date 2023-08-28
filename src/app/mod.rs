use crate::winshit::*;

use eframe::egui;
use egui::{FontId, Key};
use epaint::Vec2;
use log::{error, info};

#[derive(Default)]
pub struct MyApp {
    pub query: String,
    pub windows: Vec<WindowInfo>,
}

impl eframe::App for MyApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // Make sure we don't paint anything behind the rounded corners
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        frame.focus();
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            frame.close();
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            unsafe {
                focus_window(self.windows[0].handle);
            }
            frame.close();
        }

        custom_window_frame(ctx, |ui| {
            let max_rect = ui.max_rect();
            let te = ui.add_sized(
                [max_rect.width(), 35.0],
                egui::TextEdit::singleline(&mut self.query)
                    .font(FontId::proportional(18.0))
                    .vertical_align(egui::Align::Center),
            );
            te.request_focus();

            egui::ScrollArea::vertical()
                .max_width(f32::INFINITY)
                .show(ui, |ui| {
                    ui.add_space(2.5);
                    ui.vertical_centered_justified(|ui| {
                        self.windows.sort_by(|a, b| {
                            let a_score =
                                match sublime_fuzzy::FuzzySearch::new(&self.query, &a.title)
                                    .case_insensitive()
                                    .best_match()
                                {
                                    Some(m) => m.score(),
                                    None => 0,
                                };
                            let b_score =
                                match sublime_fuzzy::FuzzySearch::new(&self.query, &b.title)
                                    .case_insensitive()
                                    .best_match()
                                {
                                    Some(m) => m.score(),
                                    None => 0,
                                };
                            b_score.cmp(&a_score)
                        });
                        let mut top_element_set = false;
                        self.windows.iter().for_each(|window| {
                            let should_close = window_element(ui, window, !top_element_set);
                            if should_close {
                                frame.close();
                            }
                            top_element_set = true;
                        });
                    });
                });
        });
    }
}

pub fn window_element(ui: &mut egui::Ui, window: &WindowInfo, _top_element: bool) -> bool{
    let resp = ui.add(match window.icon.clone() {
        Some(icon) => {
            let texture: &egui::TextureHandle =
                &ui.ctx()
                    .load_texture(window.title.clone(), icon, Default::default());
            egui::Button::image_and_text(
                texture.into(),
                Vec2::new(24.0, 24.0),
                window.title.as_str(),
            )
        }
        None => egui::Button::new(window.title.as_str()),
    });
    if _top_element {
        resp.clone().highlight();
    }
    if resp.clicked() {
        unsafe{focus_window(window.handle);}
        return true;
    }
    false
}

pub fn custom_window_frame(
    ctx: &egui::Context,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    use egui::*;

    let panel_frame = egui::Frame {
        fill: ctx.style().visuals.window_fill(),
        rounding: 10.0.into(),
        stroke: ctx.style().visuals.widgets.noninteractive.fg_stroke,
        outer_margin: 0.5.into(), // so the stroke is within the bounds
        ..Default::default()
    };

    CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
        let app_rect = ui.max_rect();

        let content_rect = app_rect.shrink(6.0);

        let mut content_ui = ui.child_ui(content_rect, *ui.layout());
        add_contents(&mut content_ui);
    });
}
