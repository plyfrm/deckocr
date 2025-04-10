use core::f32;

use eframe::egui::{self, Color32, Widget};

use crate::word::TextWithRuby;

// TODO: text selection?
pub struct TextWithRubyWidget<'a> {
    text_with_ruby: &'a TextWithRuby,
    text_size: f32,
    ruby_size: f32,
    colour: Color32,
}

impl<'a> TextWithRubyWidget<'a> {
    pub fn new(text_with_ruby: &'a TextWithRuby) -> Self {
        Self {
            text_with_ruby,
            text_size: 11.0,
            ruby_size: 4.0,
            colour: Color32::WHITE,
        }
    }

    pub fn text_size(self, text_size: f32) -> Self {
        Self { text_size, ..self }
    }

    pub fn ruby_size(self, ruby_size: f32) -> Self {
        Self { ruby_size, ..self }
    }

    pub fn colour(self, colour: Color32) -> Self {
        Self { colour, ..self }
    }
}

impl<'a> Widget for TextWithRubyWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut job = egui::text::LayoutJob::default();

        job.wrap = egui::text::TextWrapping::truncate_at_width(ui.available_width());

        for fragment in &self.text_with_ruby.0 {
            job.append(
                &fragment.text,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(self.text_size),
                    color: self.colour,
                    ..Default::default()
                },
            );
        }

        let galley = ui.fonts(|fonts| fonts.layout_job(job));

        let contains_ruby = self
            .text_with_ruby
            .0
            .iter()
            .any(|fragment| fragment.ruby.is_some());

        let mut desired_size = galley.size();
        desired_size.y += self.ruby_size;

        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        if !contains_ruby {
            response.rect.min.y += self.ruby_size;
        }

        let mut pos = rect.left_top();
        pos.y += self.ruby_size;

        let mut clip_rect = rect;
        clip_rect.set_top(f32::NEG_INFINITY);
        clip_rect.set_bottom(f32::INFINITY);
        clip_rect.set_left(f32::NEG_INFINITY);

        for fragment in &self.text_with_ruby.0 {
            let painter = ui.painter_at(clip_rect);

            let text_rect = painter.text(
                pos,
                egui::Align2::LEFT_TOP,
                &fragment.text,
                egui::FontId::proportional(self.text_size),
                self.colour,
            );

            pos.x += text_rect.width();

            if let Some(ruby) = &fragment.ruby {
                painter.text(
                    text_rect.center_top(),
                    egui::Align2::CENTER_CENTER,
                    ruby,
                    egui::FontId::proportional(self.ruby_size),
                    self.colour,
                );
            }
        }

        response
    }
}
