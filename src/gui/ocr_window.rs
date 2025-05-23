use std::collections::HashMap;

use anyhow::{Context, Result};
use eframe::egui::{self, vec2, Color32, CornerRadius, Pos2, Rect, TextureHandle};
use egui_extras::Size;
use gilrs::Gilrs;
use image::RgbaImage;

use crate::{
    config::AppConfig,
    services::{
        dictionary::DictionaryServiceJob,
        ocr::{OcrResponse, OcrServiceJob},
        ServiceJob, Services,
    },
    word::Word,
    Popups, WINDOW_TITLE,
};

mod input_state;
use input_state::*;

mod text_with_ruby_widget;
use text_with_ruby_widget::*;

/// The OCR window, shown when the user presses the OCR hotkey.
pub struct OcrWindow {
    pub close_requested: bool,

    pub texture: TextureHandle,
    pub config: AppConfig,
    pub gilrs: Gilrs,

    pub state: State,

    pub frame_count: u32,
}

/// The `OcrWindow`'s current state.
pub enum State {
    /// Waiting on the OCR service.
    LoadingOcr(OcrServiceJob),
    /// Waiting on the dictionary service.
    LoadingDictionary(DictionaryServiceJob),
    /// Waiting on the SRS service.
    LoadingSrs {
        words: Vec<Vec<Word>>,
        job: ServiceJob<Result<()>>,
    },
    /// The UI is ready to be shown.
    Ready(ReadyState),
}

impl State {
    /// Whether we are still waiting on data from services.
    pub fn is_loading(&self) -> bool {
        match self {
            Self::LoadingOcr(_) | Self::LoadingDictionary(_) | Self::LoadingSrs { .. } => true,
            Self::Ready(_) => false,
        }
    }
}

/// The OCR window's state, after all the data has been loaded.
pub struct ReadyState {
    input_state: InputState,

    /// List of paragraphs, represented as lists of words with definitions.
    pub words: Vec<Vec<Word>>,
    /// How the words are laid out on the screen (used for finding the closest word when moving up or down).
    pub word_rects: HashMap<(usize, usize), Rect>,

    /// Index of the word currently selected by the user.
    pub selected_word: (usize, usize),
    /// Whether we should scroll to the currently selected word on this frame.
    pub scroll_to_current_word_requested: bool,

    /// Job created when the user adds a new word to their deck.
    pub add_to_deck_job: Option<ServiceJob<Result<()>>>,
}

impl ReadyState {
    /// Returns a reference to the currently selected word.
    pub fn selected_word(&self) -> &Word {
        &self.words[self.selected_word.0][self.selected_word.1]
    }

    /// Returns a mutable reference to the currently selected word.
    pub fn selected_word_mut(&mut self) -> &mut Word {
        &mut self.words[self.selected_word.0][self.selected_word.1]
    }
}

impl OcrWindow {
    /// Create a new `OcrWindow` and start querying data from services.
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

    /// Manages the `OcrWindow`'s state while it is still loading.
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
                    self.state = State::LoadingSrs {
                        job: services
                            .srs
                            .load_card_states(words.iter().flatten().cloned().collect()),
                        words,
                    };
                }
            },
            State::LoadingSrs { words, job } => match job
                .try_wait()
                .unwrap()
                .transpose()
                .context("SRS ServiceJob returned an error")?
            {
                None => {}
                Some(_) => {
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
                        words: std::mem::take(words),
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

    /// Show the window to the user.
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

        // show errors if add_to_deck_job has failed
        if let State::Ready(state) = &mut self.state {
            if let Some(job) = &mut state.add_to_deck_job {
                match job.try_wait() {
                    Ok(None) => {}
                    Ok(Some(Ok(_))) => {
                        state.add_to_deck_job = None;
                    }
                    Err(e) | Ok(Some(Err(e))) => {
                        popups.error(e);
                        state.add_to_deck_job = None;
                    }
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
                        self.show_ui(ui, services);

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

    /// Updates the window's state based on the user's input.
    fn handle_input(&mut self, ctx: &egui::Context, services: &mut Services) -> Result<()> {
        let State::Ready(state) = &mut self.state else {
            panic!("invariant broken: handle_input should only be called when self.state is Some!");
        };

        state.input_state.update(ctx, &mut self.gilrs);

        let skip_irrelevant_words = state.input_state.skip_irrelevant.is_pressed();

        let word_is_valid = |word: &Word| {
            if skip_irrelevant_words {
                services.srs.card_state(word).is_relevant
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
            state.add_to_deck_job = Some(services.srs.add_to_deck(&word));
        }

        // TODO left/right stick scrolling

        Ok(())
    }

    /// Show the inner UI of the window, once it has loaded.
    fn show_ui(&mut self, ui: &mut egui::Ui, services: &Services) {
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

                            strip.cell(|ui| text_panel_ui(self, ui, services));

                            strip.empty();

                            strip.cell(|ui| definition_panel_ui(self, ui, services));

                            strip.empty();
                        });
                });

                strip.cell(|ui| bottom_bar_ui(self, ui));
            });

        fn text_panel_ui(win: &mut OcrWindow, ui: &mut egui::Ui, services: &Services) {
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
                                let colour = {
                                    let [r, g, b] = services.srs.card_state(word).colour;
                                    Color32::from_rgb(r, g, b)
                                };

                                let rect = ui
                                    .add(
                                        TextWithRubyWidget::new(&word.text)
                                            .text_size(text_size)
                                            .ruby_size(ruby_size)
                                            .colour(colour),
                                    )
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

        fn definition_panel_ui(win: &mut OcrWindow, ui: &mut egui::Ui, services: &Services) {
            let State::Ready(state) = &mut win.state else {
                panic!("invariant broken: show_without_rects should only be called when self.state is Some!");
            };

            match &state.selected_word().definition {
                None => {}
                Some(word) => {
                    let spelling_size = 64.0;
                    let text_size = 24.0;

                    let card_state = services.srs.card_state(state.selected_word());

                    let card_colour = {
                        let [r, g, b] = card_state.colour;
                        Color32::from_rgb(r, g, b)
                    };

                    ui.columns_const(|[col1, col2]| {
                        col1.add(egui::Label::new(
                            egui::RichText::new(&card_state.name)
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
            let dpad = egui::include_image!("../../assets/controller_icons/steamdeck_dpad.svg");
            let rtrigger =
                egui::include_image!("../../assets/controller_icons/steamdeck_button_r2.svg");
            let a = egui::include_image!("../../assets/controller_icons/steamdeck_button_a.svg");
            let b = egui::include_image!("../../assets/controller_icons/steamdeck_button_b.svg");

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
