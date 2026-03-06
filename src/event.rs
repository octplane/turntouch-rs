use std::time::{Duration, Instant};

use crate::protocol::{ButtonEvent, ButtonState, Direction};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(250);

/// Tracks button state transitions and emits high-level events.
pub struct EventDetector {
    prev_state: ButtonState,
    /// For software double-click detection: last single-press direction and when it fired.
    last_press: Option<(Direction, Instant)>,
    /// Whether the current press has already been classified as held.
    hold_emitted: bool,
    /// Whether we already emitted a multi-press for the current gesture.
    multi_emitted: bool,
}

impl EventDetector {
    pub fn new() -> Self {
        Self {
            prev_state: ButtonState::default(),
            last_press: None,
            hold_emitted: false,
            multi_emitted: false,
        }
    }

    /// Process raw BLE notification bytes and return any resulting event.
    pub fn process(&mut self, data: &[u8]) -> Vec<ButtonEvent> {
        let state = ButtonState::from_bytes(data);
        let mut events = Vec::new();

        // Multi-touch (2+ buttons): emit once per gesture
        if state.pressed_count() > 1 && !self.multi_emitted {
            events.push(ButtonEvent::Multi(state.pressed_directions()));
            self.multi_emitted = true;
            self.prev_state = state;
            return events;
        }
        if state.pressed_count() > 1 {
            self.prev_state = state;
            return events;
        }

        // Button released
        if !state.any_pressed() && self.prev_state.any_pressed() {
            self.multi_emitted = false;
            if !self.hold_emitted {
                if let Some(dir) = self.prev_state.pressed_direction() {
                    // Check for hardware double-click (V3+ firmware)
                    if self.prev_state.is_double_click() {
                        events.push(ButtonEvent::DoubleTap(dir));
                        self.last_press = None;
                    }
                    // Check for software double-click
                    else if let Some((last_dir, last_time)) = self.last_press {
                        if last_dir == dir && last_time.elapsed() < DOUBLE_CLICK_WINDOW {
                            events.push(ButtonEvent::DoubleTap(dir));
                            self.last_press = None;
                        } else {
                            events.push(ButtonEvent::Press(dir));
                            self.last_press = Some((dir, Instant::now()));
                        }
                    } else {
                        events.push(ButtonEvent::Press(dir));
                        self.last_press = Some((dir, Instant::now()));
                    }
                }
            }
            self.hold_emitted = false;
        }

        // Button pressed with hold flag
        if state.any_pressed() && state.held && !self.hold_emitted {
            if let Some(dir) = state.pressed_direction() {
                events.push(ButtonEvent::Hold(dir));
                self.hold_emitted = true;
            }
        }

        // New button press (transition from not pressed to pressed)
        if state.any_pressed() && !self.prev_state.any_pressed() {
            self.hold_emitted = false;
        }

        self.prev_state = state;
        events
    }
}
