use core::f32;
use std::collections::HashMap;

use anyhow::Result;
use eframe::egui::{self, vec2, Color32, CornerRadius, Pos2, Rect, TextureHandle, Widget};
use egui_extras::Size;
use gilrs::Gilrs;
use image::RgbaImage;

use crate::{
    config::AppConfig,
    service::{
        dictionary::{DictionaryServiceJob, TextWithRuby, Word},
        ocr::{OcrResponse, OcrServiceJob},
        Services,
    },
    Errors, WINDOW_TITLE,
};

/// Holding the skip button will automatically skip to those words
const RELEVANT_CARD_STATES: &[&str] = &["not in deck", "new", "learning", "due", "failed"];

pub struct OcrWindow {
    pub close_requested: bool,

    pub texture: TextureHandle,
    pub config: AppConfig,
    pub gilrs: Gilrs,

    // still loading if some
    pub state: State,

    pub frame_count: u32,
}

pub enum State {
    LoadingOcr(OcrServiceJob),
    LoadingDictionary(DictionaryServiceJob),
    Ready(Ready),
}

impl State {
    pub fn is_loading(&self) -> bool {
        match self {
            Self::LoadingOcr(_) | Self::LoadingDictionary(_) => true,
            Self::Ready(_) => false,
        }
    }
}

pub struct Ready {
    pub words: Vec<Vec<Word>>,
    pub word_rects: HashMap<(usize, usize), Rect>, // used for finding next word on user input

    pub selected_word: (usize, usize),
    pub scroll_to_current_word_requested: bool,
}

impl Ready {
    pub fn selected_word(&self) -> &Word {
        &self.words[self.selected_word.0][self.selected_word.1]
    }

    pub fn selected_word_mut(&mut self) -> &mut Word {
        &mut self.words[self.selected_word.0][self.selected_word.1]
    }
}

impl OcrWindow {
    pub fn new(
        ctx: &egui::Context,
        config: AppConfig,
        image: RgbaImage,
        services: &mut Services,
    ) -> Result<Self> {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_flat_samples().as_slice(),
        );

        let texture = ctx.load_texture(
            "ocr window background",
            color_image,
            egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                wrap_mode: egui::TextureWrapMode::ClampToEdge,
                mipmap_mode: None,
            },
        );

        let state = State::LoadingOcr(services.ocr.ocr(image));

        Ok(Self {
            close_requested: false,

            texture,
            config,
            gilrs: Gilrs::new().unwrap(),

            state,

            frame_count: 0,
        })
    }

    pub fn manage_loading(&mut self, services: &mut Services) -> Result<()> {
        match &mut self.state {
            State::Ready(_) => {}
            State::LoadingOcr(job) => match job.try_wait().unwrap().transpose()? {
                None => {}
                Some(OcrResponse::WithRects(_)) => unimplemented!(),
                Some(OcrResponse::WithoutRects(text)) => {
                    self.state = State::LoadingDictionary(services.dictionary.parse(text));
                }
            },
            State::LoadingDictionary(job) => match job.try_wait().unwrap().transpose()? {
                None => {}
                Some(words) => {
                    // set selected word to the first word with a definition
                    let mut selected_word = (0, 0);
                    'outer: for (i, paragraph) in words.iter().enumerate() {
                        for (j, word) in paragraph.iter().enumerate() {
                            if word.definition.is_some() {
                                selected_word = (i, j);
                                break 'outer;
                            }
                        }
                    }
                    self.state = State::Ready(Ready {
                        words,
                        word_rects: Default::default(),
                        selected_word,
                        scroll_to_current_word_requested: false,
                    });
                }
            },
        }

        Ok(())
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        config: &AppConfig,
        errors: &mut Errors,
        services: &mut Services,
    ) {
        if let Err(e) = self.manage_loading(services) {
            errors.push(e);
            // we need to close the ocr window immediately when this errors, or we'll keep attempting to wait
            // on service jobs which have already finished with an error
            self.close_requested = true;
        }

        // NOTE: the viewport needs to be fully closed for at least 1 frame or we aren't
        // able to grab the focus again
        if self.frame_count == 0 {
            self.frame_count += 1;
            return;
        }

        ctx.show_viewport_immediate(
            egui::ViewportId(egui::Id::new("ocr_viewport")),
            egui::ViewportBuilder {
                title: Some(WINDOW_TITLE.to_owned()),
                inner_size: match self.config.fullscreen {
                    true => Some(self.texture.size_vec2()),
                    false => Some(vec2(
                        config.window_width as f32,
                        config.window_height as f32,
                    )),
                },
                fullscreen: Some(self.config.fullscreen),
                ..Default::default()
            },
            |ctx, _| {
                if self.frame_count == 1 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.painter().image(
                        self.texture.id(),
                        ctx.available_rect(),
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.painter().rect_filled(
                        ctx.available_rect(),
                        CornerRadius::ZERO,
                        Color32::from_black_alpha(self.config.background_dimming),
                    );

                    if self.state.is_loading() {
                        ui.centered_and_justified(|ui| {
                            ui.add(
                                egui::Spinner::new()
                                    .color(Color32::from_white_alpha(96))
                                    .size(48.0),
                            );
                        });
                    } else {
                        self.show_without_rects(ui);

                        if let Err(e) = self.handle_input(ctx, services) {
                            errors.push(e);
                        }
                    }

                    ctx.input(|input| {
                        if input.viewport().close_requested() {
                            self.close_requested = true;
                        }
                    });
                });
            },
        );

        self.frame_count += 1;
    }

    // TODO: controller input
    // TODO: allow scrolling the text and definition panes
    // TODO: clean up that whole function tbh
    fn handle_input(&mut self, ctx: &egui::Context, services: &mut Services) -> Result<()> {
        let State::Ready(state) = &mut self.state else {
            panic!("invariant broken: handle_input should only be called when self.state is Some!");
        };

        fn move_h(
            direction: i32,
            selected_word: (usize, usize),
            words: &Vec<Vec<Word>>,
            _word_rects: &HashMap<(usize, usize), Rect>,
        ) -> Option<(usize, usize)> {
            let mut cursor = (selected_word.0 as i32, selected_word.1 as i32);
            loop {
                cursor.1 += direction;
                if cursor.1 < 0 {
                    cursor.0 = i32::max(0, cursor.0 - 1);
                    cursor.1 = words[cursor.0 as usize].len() as i32 - 1;
                } else if cursor.1 >= words[cursor.0 as usize].len() as i32 {
                    cursor.0 = i32::min(cursor.0 + 1, words.len() as i32 - 1);
                    cursor.1 = 0;
                }

                if words[cursor.0 as usize][cursor.1 as usize]
                    .definition
                    .is_some()
                {
                    return Some((cursor.0 as usize, cursor.1 as usize));
                }

                if cursor == (0, 0) {
                    return None;
                } else if (words.len() - 1, words.last().unwrap().len() - 1)
                    == (cursor.0 as usize, cursor.1 as usize)
                {
                    return None;
                }
            }
        }

        fn move_v(
            direction: i32,
            selected_word: (usize, usize),
            words: &Vec<Vec<Word>>,
            word_rects: &HashMap<(usize, usize), Rect>,
        ) -> Option<(usize, usize)> {
            let current_rect = word_rects.get(&selected_word).copied().unwrap();

            let filter = |(_, rect): &(_, &Rect)| {
                if direction.is_negative() {
                    rect.bottom() < current_rect.bottom()
                } else {
                    rect.bottom() > current_rect.bottom()
                }
            };

            word_rects
                .iter()
                .filter(|(idx, _)| words[idx.0][idx.1].definition.is_some())
                .filter(filter)
                .map(|(idx, rect)| (idx, rect.center().distance(current_rect.center())))
                .min_by(|(_, dist1), (_, dist2)| dist1.total_cmp(dist2))
                .map(|(idx, _)| *idx)
        }

        // ugly ass autoformat
        let mut skip_irrelevant = |should_skip: bool,
                                   direction: i32,
                                   f: fn(
            i32,
            (usize, usize),
            &Vec<Vec<Word>>,
            &HashMap<(usize, usize), Rect>,
        ) -> Option<(usize, usize)>| {
            if !should_skip {
                f(
                    direction,
                    state.selected_word,
                    &state.words,
                    &state.word_rects,
                )
                .map(|idx| state.selected_word = idx);
            } else {
                // TODO: make vertical skip irrelevant more intuitive
                let word_count = state.words.iter().map(|v| v.iter()).flatten().count();
                // NOTE: the upper bound is a workaround for the alg going into an infinite loop sometimes. need to revise this at some point
                for _ in 0..word_count {
                    let new = f(
                        direction,
                        state.selected_word,
                        &state.words,
                        &state.word_rects,
                    )
                    .map(|idx| state.selected_word = idx);
                    if new.is_none() {
                        break;
                    }
                    if RELEVANT_CARD_STATES.contains(
                        &state.words[state.selected_word.0][state.selected_word.1]
                            .definition
                            .as_ref()
                            .unwrap()
                            .card_state
                            .as_str(),
                    ) {
                        break;
                    }
                }
            }
        };

        let should_skip = ctx.input(|input| {
            input.modifiers.shift
                || input.pointer.button_down(egui::PointerButton::Primary)
                || self
                    .gilrs
                    .gamepads()
                    .nth(0)
                    .map(|gamepad| gamepad.1.is_pressed(gilrs::Button::RightTrigger2))
                    .unwrap_or(false)
        });

        let mut add_to_deck = false;
        state.scroll_to_current_word_requested = false;

        ctx.input(|input| {
            if input.key_pressed(egui::Key::ArrowLeft) {
                skip_irrelevant(should_skip, -1, move_h);
                state.scroll_to_current_word_requested = true;
            }
            if input.key_pressed(egui::Key::ArrowRight) {
                skip_irrelevant(should_skip, 1, move_h);
                state.scroll_to_current_word_requested = true;
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                skip_irrelevant(should_skip, -1, move_v);
                state.scroll_to_current_word_requested = true;
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                skip_irrelevant(should_skip, 1, move_v);
                state.scroll_to_current_word_requested = true;
            }
            if input.key_pressed(egui::Key::Escape) {
                self.close_requested = true;
            }
            if input.key_pressed(egui::Key::Enter) {
                add_to_deck = true;
            }
        });

        if add_to_deck {
            let word = state.words[state.selected_word.0][state.selected_word.1].clone();

            services.srs.add_to_deck(&word).wait()??;

            state
                .selected_word_mut()
                .definition
                .as_mut()
                .unwrap()
                .card_state = "new".to_owned();
        }

        Ok(())
    }

    fn _show_with_rects(&mut self, _ui: &mut egui::Ui) {
        unimplemented!()
    }

    fn show_without_rects(&mut self, ui: &mut egui::Ui) {
        let padding_h = 32.0;
        let padding_v = padding_h / 2.0;
        let bottom_bar = 64.0;
        let definition_panel = 400.0;

        egui_extras::StripBuilder::new(ui)
            .size(Size::exact(padding_v))
            .size(Size::remainder())
            .size(Size::exact(bottom_bar))
            .vertical(|mut strip| {
                strip.empty();

                strip.strip(|builder| {
                    builder
                        .size(Size::exact(padding_h))
                        .size(Size::remainder())
                        .size(Size::exact(padding_h))
                        .size(Size::exact(definition_panel))
                        .size(Size::exact(padding_h))
                        .horizontal(|mut strip| {
                            strip.empty();

                            strip.cell(|ui| text_panel_ui(self, ui));

                            strip.empty();

                            strip.cell(|ui| definition_panel_ui(self, ui));

                            strip.empty();
                        });
                });

                strip.cell(|ui| bottom_bar_ui(self, ui));
            });

        fn text_panel_ui(win: &mut OcrWindow, ui: &mut egui::Ui) {
            let State::Ready(state) = &mut win.state else {
                panic!("invariant broken: show_without_rects should only be called when self.state is Some!");
            };

            egui::ScrollArea::vertical().show(ui, |ui| {
                let text_size = 32.0;
                let ruby_size = 11.0;
                let selection_highlight = Color32::from_white_alpha(8);
                let paragraph_spacing = text_size / 2.0;

                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                let mut word_rects = HashMap::new();

                for (paragraph_idx, paragraph) in state.words.iter().enumerate() {
                    if paragraph_idx == state.selected_word.0 {
                        ui.add_space(paragraph_spacing);
                    }

                    ui.horizontal_wrapped(|ui| {
                        for (word_idx, word) in paragraph.iter().enumerate() {
                            let colour = word
                                .definition
                                .as_ref()
                                .map(|def| win.config.card_colours.get(&def.card_state))
                                .flatten()
                                .copied()
                                .map(|[r, g, b]| Color32::from_rgb(r, g, b))
                                .unwrap_or(Color32::WHITE);

                            let rect = ui
                                .add(TextWithRubyWidget {
                                    text_with_ruby: &word.text,
                                    text_size,
                                    ruby_size,
                                    colour,
                                })
                                .rect;

                            if state.word_rects.is_empty() {
                                word_rects.insert((paragraph_idx, word_idx), rect);
                            }

                            if state.selected_word == (paragraph_idx, word_idx) {
                                if state.scroll_to_current_word_requested {
                                    ui.scroll_to_rect(rect, None);
                                }
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::ZERO,
                                    selection_highlight,
                                );
                            }
                        }
                    });

                    if paragraph_idx == state.selected_word.0 {
                        ui.add_space(paragraph_spacing);
                    }
                    ui.add_space(paragraph_spacing);
                }

                if state.word_rects.is_empty() {
                    state.word_rects = word_rects;
                }
            });
        }

        fn definition_panel_ui(win: &mut OcrWindow, ui: &mut egui::Ui) {
            let State::Ready(state) = &mut win.state else {
                panic!("invariant broken: show_without_rects should only be called when self.state is Some!");
            };

            match &state.selected_word().definition {
                None => {}
                Some(word) => {
                    let spelling_size = 64.0;
                    let text_size = 24.0;

                    let card_colour = win
                        .config
                        .card_colours
                        .get(&word.card_state)
                        .map(|&[r, g, b]| Color32::from_rgb(r, g, b))
                        .unwrap_or(Color32::WHITE);

                    ui.columns_const(|[col1, col2]| {
                        col1.add(egui::Label::new(
                            egui::RichText::new(&word.card_state)
                                .size(text_size)
                                .color(card_colour),
                        ));

                        let freq = word
                            .frequency
                            .map(|n| format!("Top {n}"))
                            .unwrap_or_else(|| "Unknown Frequency".to_owned());

                        col2.add(egui::Label::new(
                            egui::RichText::new(freq)
                                .size(text_size)
                                .color(Color32::WHITE),
                        ));
                    });

                    ui.add(egui::Label::new(
                        egui::RichText::new(&word.spelling)
                            .size(spelling_size)
                            .color(Color32::WHITE),
                    ));

                    ui.add(egui::Label::new(
                        egui::RichText::new(&word.reading)
                            .size(text_size)
                            .color(Color32::from_white_alpha(192)),
                    ));

                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for meaning in &word.meanings {
                            ui.add(egui::Label::new(
                                egui::RichText::new(format!("・{meaning}"))
                                    .size(text_size)
                                    .color(Color32::WHITE),
                            ));
                        }
                    });
                }
            }
        }

        fn bottom_bar_ui(_win: &mut OcrWindow, ui: &mut egui::Ui) {
            let dpad = egui::include_image!("../assets/controller_icons/shared/sharerd-D-PAD.svg");
            let rtrigger =
                egui::include_image!("../assets/controller_icons/shared/shared-Right Trigger.svg");
            let a = egui::include_image!("../assets/controller_icons/shared/shared-A.svg");
            let b = egui::include_image!("../assets/controller_icons/shared/shared-B.svg");

            let glyph_size = 32.0;
            let text_size = 20.0;
            let spacing = 24.0;

            let add_glyph = |ui: &mut egui::Ui, glyph| {
                ui.add(egui::Image::new(glyph).fit_to_exact_size(vec2(glyph_size, glyph_size)));
            };

            let add_label = |ui: &mut egui::Ui, text| {
                ui.add(egui::Label::new(
                    egui::RichText::new(text)
                        .size(text_size)
                        .color(Color32::WHITE),
                ));
            };

            // pushing things downwards a little bit
            ui.add_space(8.0);

            ui.horizontal_centered(|ui| {
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center).with_cross_justify(true),
                    |ui| {
                        ui.add_space(spacing);
                        add_glyph(ui, dpad);
                        ui.add_space(spacing / 2.0);
                        add_label(ui, "MOVE SELECTION");

                        ui.add_space(spacing);
                        add_glyph(ui, rtrigger);
                        ui.add_space(spacing / 2.0);
                        add_label(ui, "SKIP IRRELEVANT WORDS");
                    },
                );

                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center).with_cross_justify(true),
                    |ui| {
                        ui.add_space(spacing);
                        add_label(ui, "EXIT");
                        ui.add_space(spacing / 2.0);
                        add_glyph(ui, b);

                        ui.add_space(spacing);
                        add_label(ui, "ADD TO DECK");
                        ui.add_space(spacing / 2.0);
                        add_glyph(ui, a);
                    },
                );
            });
        }
    }
}

// TODO: text selection?
struct TextWithRubyWidget<'a> {
    text_with_ruby: &'a TextWithRuby,
    text_size: f32,
    ruby_size: f32,
    colour: Color32,
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

pub fn fill_texture(ctx: &egui::Context, ui: &mut egui::Ui, texture: &TextureHandle) {}
