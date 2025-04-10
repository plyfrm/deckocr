use core::f32;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use eframe::egui::{self, vec2, Color32, CornerRadius, Pos2, Rect, TextureHandle, Widget};
use egui_extras::Size;
use gilrs::Gilrs;
use image::RgbaImage;

use crate::{
    config::AppConfig,
    service::{
        dictionary::{DictionaryServiceJob, TextWithRuby, Word},
        ocr::{OcrResponse, OcrServiceJob},
        ServiceJob, Services,
    },
    Popups, WINDOW_TITLE,
};

/// Holding the skip button will automatically skip to those words
const RELEVANT_CARD_STATES: &[&str] = &["not in deck", "new", "learning", "due", "failed"];

pub struct OcrWindow {
    pub close_requested: bool,

    pub texture: TextureHandle,
    pub config: AppConfig,
    pub gilrs: Gilrs,

    pub state: State,

    pub frame_count: u32,
}

pub enum State {
    LoadingOcr(OcrServiceJob),
    LoadingDictionary(DictionaryServiceJob),
    Ready(ReadyState),
}

impl State {
    pub fn is_loading(&self) -> bool {
        match self {
            Self::LoadingOcr(_) | Self::LoadingDictionary(_) => true,
            Self::Ready(_) => false,
        }
    }
}

pub struct ReadyState {
    input_state: InputState,

    pub words: Vec<Vec<Word>>,
    pub word_rects: HashMap<(usize, usize), Rect>, // used for finding next word on user input

    pub selected_word: (usize, usize),
    pub scroll_to_current_word_requested: bool,

    pub add_to_deck_job: Option<((usize, usize), ServiceJob<Result<()>>)>,
}

impl ReadyState {
    pub fn selected_word(&self) -> &Word {
        &self.words[self.selected_word.0][self.selected_word.1]
    }

    pub fn selected_word_mut(&mut self) -> &mut Word {
        &mut self.words[self.selected_word.0][self.selected_word.1]
    }
}

#[derive(Debug)]
struct Key {
    is_pressed: Option<Instant>,
    was_consumed: bool,
    last_retriggered: Instant,
}

impl Key {
    fn change_state(&mut self, is_pressed: bool) {
        if !is_pressed && self.is_pressed.is_some() {
            self.is_pressed = None;
        }

        if is_pressed && self.is_pressed.is_none() {
            self.is_pressed = Some(Instant::now());
            self.was_consumed = false;
        }
    }

    fn is_pressed(&self) -> bool {
        self.is_pressed.is_some()
    }

    fn was_pressed(&mut self) -> bool {
        if self.is_pressed.is_some() && !self.was_consumed {
            self.was_consumed = true;
            true
        } else {
            false
        }
    }

    fn was_pressed_with_retrigger(&mut self) -> bool {
        let delay_before_first_retrigger = Duration::from_millis(300);
        let delay_between_retriggers = Duration::from_millis(50);

        if let Some(pressed_timestamp) = self.is_pressed {
            if !self.was_consumed {
                self.was_consumed = true;
                return true;
            } else if pressed_timestamp.elapsed() > delay_before_first_retrigger
                && self.last_retriggered.elapsed() > delay_between_retriggers
            {
                self.last_retriggered = Instant::now();
                return true;
            }
        }

        false
    }
}

impl Default for Key {
    fn default() -> Self {
        Self {
            is_pressed: None,
            was_consumed: false,
            last_retriggered: Instant::now(),
        }
    }
}

#[derive(Debug, Default)]
struct InputState {
    up: Key,
    down: Key,
    left: Key,
    right: Key,
    skip_irrelevant: Key,
    add_to_deck: Key,
    exit: Key,
    scroll_left: f32,
    scroll_right: f32,
}

impl InputState {
    pub fn update(&mut self, ctx: &egui::Context, gilrs: &mut Gilrs) {
        let gilrs_events: Vec<_> = std::iter::from_fn(|| gilrs.next_event())
            .map(|e| e.event)
            .collect();

        let update_key = |key: &mut Key, egui_key: egui::Key, gilrs_button: gilrs::Button| {
            let mut is_pressed = false;

            is_pressed |= ctx.input(|input| input.key_down(egui_key));
            is_pressed |= gilrs
                .gamepads()
                .any(|(_, gamepad)| gamepad.is_pressed(gilrs_button));

            key.change_state(is_pressed);
        };

        {
            use egui::Key as K;
            use gilrs::Button as B;

            update_key(&mut self.up, K::ArrowUp, B::DPadUp);
            update_key(&mut self.down, K::ArrowDown, B::DPadDown);
            update_key(&mut self.left, K::ArrowLeft, B::DPadLeft);
            update_key(&mut self.right, K::ArrowRight, B::DPadRight);
            update_key(&mut self.add_to_deck, K::Enter, B::South);
            update_key(&mut self.exit, K::Escape, B::East);
        }

        let skip_irrelevant_pressed = ctx.input(|input| {
            input.modifiers.shift || input.pointer.button_down(egui::PointerButton::Primary)
        }) || gilrs
            .gamepads()
            .any(|(_, gamepad)| gamepad.is_pressed(gilrs::Button::RightTrigger2));

        self.skip_irrelevant.change_state(skip_irrelevant_pressed);

        for event in gilrs_events {
            match event {
                gilrs::EventType::AxisChanged(gilrs::Axis::LeftStickY, value, _) => {
                    self.scroll_left = value
                }
                gilrs::EventType::AxisChanged(gilrs::Axis::RightStickY, value, _) => {
                    self.scroll_right = value
                }
                _ => {}
            }
        }
    }
}

impl OcrWindow {
    pub fn new(
        ctx: &egui::Context,
        config: AppConfig,
        image: RgbaImage,
        services: &mut Services,
    ) -> Self {
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

        Self {
            close_requested: false,

            texture,
            config,
            gilrs: Gilrs::new().unwrap(),

            state,

            frame_count: 0,
        }
    }

    pub fn is_loading(&self) -> bool {
        match &self.state {
            State::LoadingDictionary(_) | State::LoadingOcr(_) => true,
            State::Ready(_) => false,
        }
    }

    pub fn manage_loading(&mut self, services: &mut Services) -> Result<()> {
        match &mut self.state {
            State::Ready(_) => {}
            State::LoadingOcr(job) => match job
                .try_wait()
                .unwrap()
                .transpose()
                .context("OCR ServiceJob returned an error")?
            {
                None => {}
                Some(OcrResponse::WithRects(_)) => unimplemented!(),
                Some(OcrResponse::WithoutRects(text)) => {
                    self.state = State::LoadingDictionary(services.dictionary.parse(text));
                }
            },
            State::LoadingDictionary(job) => match job
                .try_wait()
                .unwrap()
                .transpose()
                .context("Dictionary ServiceJob returned an error")?
            {
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
                    self.state = State::Ready(ReadyState {
                        input_state: Default::default(),
                        words,
                        word_rects: Default::default(),
                        selected_word,
                        scroll_to_current_word_requested: false,
                        add_to_deck_job: None,
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
        popups: &mut Popups,
        services: &mut Services,
    ) {
        if let Err(e) = self.manage_loading(services) {
            popups.error(e);
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

        // update card state if a card was added to the user's deck
        if let State::Ready(state) = &mut self.state {
            if let Some(((paragraph_idx, word_idx), job)) = &mut state.add_to_deck_job {
                match job.try_wait() {
                    Ok(Some(_)) => {
                        state.words[*paragraph_idx][*word_idx]
                            .definition
                            .as_mut()
                            .unwrap()
                            .card_state = "new".to_owned();
                        state.add_to_deck_job = None;
                    }
                    Err(e) => {
                        popups.error(e);
                        state.add_to_deck_job = None;
                    }
                    _ => {}
                }
            }
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
                            popups.error(e);
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

    fn handle_input(&mut self, ctx: &egui::Context, services: &mut Services) -> Result<()> {
        let State::Ready(state) = &mut self.state else {
            panic!("invariant broken: handle_input should only be called when self.state is Some!");
        };

        state.input_state.update(ctx, &mut self.gilrs);

        let skip_irrelevant_words = state.input_state.skip_irrelevant.is_pressed();

        let word_is_valid = |word: &Word| {
            if skip_irrelevant_words {
                word.definition
                    .as_ref()
                    .map(|definition| {
                        RELEVANT_CARD_STATES.contains(&definition.card_state.as_str())
                    })
                    .unwrap_or(false)
            } else {
                word.definition.is_some()
            }
        };

        fn checked_add(n: &mut usize, delta: i32, max_exclusive: usize) -> bool {
            let ok = (0..max_exclusive as i32).contains(&(*n as i32 + delta));
            if ok {
                *n = (*n as i32 + delta) as usize;
            }
            !ok
        }

        let move_h = |state: &mut ReadyState, delta| {
            let mut cursor = state.selected_word;
            loop {
                let overflowed = checked_add(&mut cursor.1, delta, state.words[cursor.0].len());

                if overflowed && delta.is_negative() {
                    if cursor.0 == 0 {
                        break;
                    } else {
                        cursor.0 = cursor.0.saturating_sub((-delta) as usize);
                        cursor.1 = state.words[cursor.0].len() - 1;
                    }
                }
                if overflowed && delta.is_positive() {
                    if cursor.0 == state.words.len() - 1 {
                        break;
                    } else {
                        cursor.0 = usize::min(state.words.len() - 1, cursor.0 + 1);
                        cursor.1 = 0;
                    }
                }

                if word_is_valid(&state.words[cursor.0][cursor.1]) {
                    state.selected_word = cursor;
                    break;
                }
            }
        };

        let move_v = |state: &mut ReadyState, direction: i32| {
            let current_rect = state.word_rects.get(&state.selected_word).copied().unwrap();

            state
                .word_rects
                .iter()
                .filter(|(idx, _)| state.words[idx.0][idx.1].definition.is_some())
                .filter(|(_, rect)| {
                    if direction.is_negative() {
                        rect.bottom() < current_rect.bottom()
                    } else {
                        rect.bottom() > current_rect.bottom()
                    }
                })
                .map(|(idx, rect)| (idx, rect.center().distance(current_rect.center())))
                .min_by(|(_, dist1), (_, dist2)| dist1.total_cmp(dist2))
                .map(|(idx, _)| state.selected_word = *idx);
        };

        state.scroll_to_current_word_requested = false;

        if state.input_state.left.was_pressed_with_retrigger() {
            move_h(state, -1);
            state.scroll_to_current_word_requested = true;
        }

        if state.input_state.right.was_pressed_with_retrigger() {
            move_h(state, 1);
            state.scroll_to_current_word_requested = true;
        }

        if state.input_state.up.was_pressed_with_retrigger() {
            move_v(state, -1);
            if state.input_state.skip_irrelevant.is_pressed() {
                move_h(state, -1);
            }
            state.scroll_to_current_word_requested = true;
        }

        if state.input_state.down.was_pressed_with_retrigger() {
            move_v(state, 1);
            if state.input_state.skip_irrelevant.is_pressed() {
                move_h(state, 1);
            }
            state.scroll_to_current_word_requested = true;
        }

        if state.input_state.exit.was_pressed() {
            self.close_requested = true;
        }

        if state.input_state.add_to_deck.was_pressed() {
            let word = state.selected_word().clone();
            let job = services.srs.add_to_deck(&word);
            state.add_to_deck_job = Some((state.selected_word, job));
        }

        // TODO left/right stick scrolling

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

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
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
            let dpad = egui::include_image!("../assets/controller_icons/steamdeck_dpad.svg");
            let rtrigger =
                egui::include_image!("../assets/controller_icons/steamdeck_button_r2.svg");
            let a = egui::include_image!("../assets/controller_icons/steamdeck_button_a.svg");
            let b = egui::include_image!("../assets/controller_icons/steamdeck_button_b.svg");

            let glyph_size = 48.0;
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
                        add_label(ui, "MOVE SELECTION");

                        ui.add_space(spacing);
                        add_glyph(ui, rtrigger);
                        add_label(ui, "SKIP IRRELEVANT WORDS");
                    },
                );

                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center).with_cross_justify(true),
                    |ui| {
                        ui.add_space(spacing);
                        add_label(ui, "EXIT");
                        add_glyph(ui, b);

                        ui.add_space(spacing);
                        add_label(ui, "ADD TO DECK");
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
