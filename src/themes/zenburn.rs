use eframe::egui::{self, Color32};

pub(super) fn visuals() -> egui::Visuals {
    let background = Color32::from_rgb(63, 63, 63);
    let field_background = Color32::from_rgb(38, 38, 38);
    let widget_background = Color32::from_rgb(79, 79, 79);
    let hover_background = Color32::from_rgb(95, 127, 95);
    let selection_background = Color32::from_rgb(112, 144, 128);
    let foreground = Color32::from_rgb(220, 220, 204);
    let border = Color32::from_rgb(111, 111, 111);
    let accent = Color32::from_rgb(140, 208, 211);
    let mut visuals = egui::Visuals::dark();

    visuals.override_text_color = Some(foreground);
    visuals.panel_fill = background;
    visuals.window_fill = background;
    visuals.extreme_bg_color = field_background;
    visuals.faint_bg_color = widget_background;
    visuals.code_bg_color = field_background;
    visuals.hyperlink_color = accent;
    visuals.selection.bg_fill = selection_background;
    visuals.selection.stroke = egui::Stroke::new(1.0, foreground);
    visuals.warn_fg_color = Color32::from_rgb(240, 223, 175);
    visuals.error_fg_color = Color32::from_rgb(204, 147, 147);
    visuals.window_stroke = egui::Stroke::new(1.0, border);

    visuals.widgets.noninteractive.bg_fill = background;
    visuals.widgets.noninteractive.weak_bg_fill = background;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, foreground);
    visuals.widgets.inactive.bg_fill = widget_background;
    visuals.widgets.inactive.weak_bg_fill = widget_background;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, foreground);
    visuals.widgets.hovered.bg_fill = hover_background;
    visuals.widgets.hovered.weak_bg_fill = hover_background;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, accent);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, foreground);
    visuals.widgets.active.bg_fill = selection_background;
    visuals.widgets.active.weak_bg_fill = selection_background;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, accent);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, foreground);
    visuals.widgets.open.bg_fill = widget_background;
    visuals.widgets.open.weak_bg_fill = widget_background;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, border);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, foreground);

    visuals
}
