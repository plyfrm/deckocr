use std::time::{Duration, Instant};

use eframe::egui;
use gilrs::Gilrs;

#[derive(Debug, Default)]
pub struct InputState {
    pub up: Key,
    pub down: Key,
    pub left: Key,
    pub right: Key,
    pub skip_irrelevant: Key,
    pub add_to_deck: Key,
    pub exit: Key,
    pub scroll_left: f32,
    pub scroll_right: f32,
}

impl InputState {
    pub fn update(&mut self, ctx: &egui::Context, gilrs: &mut Gilrs) {
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

        while let Some(event) = gilrs.next_event() {
            match event.event {
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

#[derive(Debug)]
pub struct Key {
    is_pressed: Option<Instant>,
    was_consumed: bool,
    last_retriggered: Instant,
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

    pub fn is_pressed(&self) -> bool {
        self.is_pressed.is_some()
    }

    pub fn was_pressed(&mut self) -> bool {
        if self.is_pressed.is_some() && !self.was_consumed {
            self.was_consumed = true;
            true
        } else {
            false
        }
    }

    pub fn was_pressed_with_retrigger(&mut self) -> bool {
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
