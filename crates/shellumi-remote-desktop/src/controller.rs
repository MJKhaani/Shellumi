//! Per-session remote desktop input controller, state tracker, and diagnostics counters.

use crate::input::{
    CanvasTransform, InputEnvelope, KeyModifiers, MouseButton, RemoteInput, RemoteKey,
    RemoteShortcut,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Atomic runtime view-only policy state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ViewOnlyPolicyState {
    #[default]
    Interactive,
    EnteringViewOnly {
        transition_id: u64,
    },
    ViewOnly,
    LeavingViewOnly {
        transition_id: u64,
    },
    SafeBlocked {
        reason: String,
    },
}

/// Authoritative worker-side ledger of input state actually submitted to the transport.
///
/// Updated *only* after successful transport submission so `ReleaseAll` and disconnect
/// recovery release every key and button that actually reached the wire.
#[derive(Debug, Clone, Default)]
pub struct SubmittedInputLedger {
    pub held_buttons: u8,
    pub last_pointer: (u32, u32),
    pub held_keys: Vec<RemoteKey>,
    pub submitted_count: u64,
}

impl SubmittedInputLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successfully submitted `RemoteInput` in the authoritative worker ledger.
    pub fn record_submitted(&mut self, input: &RemoteInput) {
        self.submitted_count = self.submitted_count.saturating_add(1);
        match input {
            RemoteInput::PointerMove { x, y } => {
                self.last_pointer = (*x, *y);
            }
            RemoteInput::PointerButton { button, down, x, y } => {
                self.last_pointer = (*x, *y);
                let mask = button.bit_mask();
                if *down {
                    self.held_buttons |= mask;
                } else {
                    self.held_buttons &= !mask;
                }
            }
            RemoteInput::PhysicalKey { key, down, .. } => {
                if *down {
                    if !self.held_keys.iter().any(|k| {
                        k.keysym == key.keysym
                            && k.scancode == key.scancode
                            && k.extended == key.extended
                    }) {
                        self.held_keys.push(key.clone());
                    }
                } else {
                    self.held_keys.retain(|k| {
                        !(k.keysym == key.keysym
                            && k.scancode == key.scancode
                            && k.extended == key.extended)
                    });
                }
            }
            RemoteInput::ReleaseAll => {
                self.held_buttons = 0;
                self.held_keys.clear();
            }
            RemoteInput::Scroll { x, y, .. } => {
                self.last_pointer = (*x, *y);
            }
            RemoteInput::CommittedText(_) | RemoteInput::Shortcut(_) => {}
        }
    }

    /// Drain all currently held buttons and keys into deterministic release events:
    /// 1. Held mouse buttons
    /// 2. Non-modifier keys in reverse press order
    /// 3. Modifier keys in reverse press order
    pub fn drain_releases(&mut self) -> Vec<RemoteInput> {
        let mut out = Vec::new();
        let (lx, ly) = self.last_pointer;
        for (button, mask) in [
            (MouseButton::Left, crate::input::pointer_mask::BUTTON_LEFT),
            (
                MouseButton::Middle,
                crate::input::pointer_mask::BUTTON_MIDDLE,
            ),
            (MouseButton::Right, crate::input::pointer_mask::BUTTON_RIGHT),
        ] {
            if (self.held_buttons & mask) != 0 {
                out.push(RemoteInput::PointerButton {
                    button,
                    down: false,
                    x: lx,
                    y: ly,
                });
            }
        }
        self.held_buttons = 0;

        let keys: Vec<RemoteKey> = std::mem::take(&mut self.held_keys);
        for key in keys.iter().rev().filter(|k| !k.is_modifier) {
            out.push(RemoteInput::PhysicalKey {
                key: key.clone(),
                down: false,
                repeat: false,
            });
        }
        for key in keys.iter().rev().filter(|k| k.is_modifier) {
            out.push(RemoteInput::PhysicalKey {
                key: key.clone(),
                down: false,
                repeat: false,
            });
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.held_buttons == 0 && self.held_keys.is_empty()
    }
}

/// Negotiated and policy-gated input capabilities for a remote session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCapabilities {
    pub supports_horizontal_wheel: bool,
    pub supports_extended_buttons: bool,
    pub supports_unicode_text: bool,
    pub supports_shortcuts: bool,
}

impl Default for InputCapabilities {
    fn default() -> Self {
        Self {
            supports_horizontal_wheel: true,
            supports_extended_buttons: false,
            supports_unicode_text: true,
            supports_shortcuts: true,
        }
    }
}

/// Reason an input event was rejected before transport dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputRejectReason {
    Disconnected,
    ViewOnly,
    StaleGeneration,
    NoGeometry,
    OutsideImage,
    Unsupported,
    QueueFull,
}

/// Structured, secret-free counters for the remote desktop input pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputPipelineCounters {
    pub ui_received: u64,
    pub accepted: u64,
    pub rejected_view_only: u64,
    pub rejected_disconnected: u64,
    pub rejected_stale_gen: u64,
    pub rejected_no_geometry: u64,
    pub rejected_outside_image: u64,
    pub rejected_unsupported: u64,
    pub coalesced_moves: u64,
    pub enqueued: u64,
    pub queue_full_errors: u64,
    pub worker_consumed: u64,
    pub transport_submitted: u64,
    pub transport_failed: u64,
    pub queue_high_water: usize,
    pub last_error_category: Option<String>,
}

/// Per-session authoritative input controller keyed by `(tab_id, connection_generation, transport_epoch)`.
#[derive(Debug, Clone)]
pub struct RemoteInputController {
    pub tab_id: u64,
    pub connection_generation: u64,
    pub transport_epoch: u64,
    pub connected: bool,
    pub focused: bool,
    pub view_only: bool,
    pub policy_state: ViewOnlyPolicyState,
    pub capabilities: InputCapabilities,
    pub held_buttons: u8,
    pub dragging_button: Option<MouseButton>,
    pub last_pointer: Option<(u32, u32)>,
    pub held_keys: HashMap<String, RemoteKey>,
    pub modifiers: KeyModifiers,
    pub scroll_acc_x: f64,
    pub scroll_acc_y: f64,
    pub layout_epoch: u64,
    pub frame_size: Option<(u32, u32)>,
    pub command_seq: u64,
    pub counters: InputPipelineCounters,
}

impl RemoteInputController {
    /// Standard Windows/RDP wheel delta per detent.
    pub const WHEEL_DELTA: f64 = 120.0;

    /// Create a new controller for a remote desktop tab.
    pub fn new(tab_id: u64, view_only: bool, capabilities: InputCapabilities) -> Self {
        Self {
            tab_id,
            connection_generation: 0,
            transport_epoch: 0,
            connected: false,
            focused: false,
            view_only,
            policy_state: if view_only {
                ViewOnlyPolicyState::ViewOnly
            } else {
                ViewOnlyPolicyState::Interactive
            },
            capabilities,
            held_buttons: 0,
            dragging_button: None,
            last_pointer: None,
            held_keys: HashMap::new(),
            modifiers: KeyModifiers::default(),
            scroll_acc_x: 0.0,
            scroll_acc_y: 0.0,
            layout_epoch: 0,
            frame_size: None,
            command_seq: 0,
            counters: InputPipelineCounters::default(),
        }
    }

    /// Reset all held state and geometry for a new connection generation or transport epoch.
    pub fn reset_for_transport(&mut self, generation: u64, transport_epoch: u64) {
        self.connection_generation = generation;
        self.transport_epoch = transport_epoch;
        self.held_buttons = 0;
        self.dragging_button = None;
        self.last_pointer = None;
        self.held_keys.clear();
        self.modifiers = KeyModifiers::default();
        self.scroll_acc_x = 0.0;
        self.scroll_acc_y = 0.0;
        self.frame_size = None;
    }

    /// Update geometry snapshot validity.
    pub fn update_geometry(&mut self, layout_epoch: u64, frame_size: Option<(u32, u32)>) {
        self.layout_epoch = layout_epoch;
        self.frame_size = frame_size;
    }

    fn check_interactive(&mut self) -> Result<(), InputRejectReason> {
        if !self.connected {
            self.counters.rejected_disconnected += 1;
            return Err(InputRejectReason::Disconnected);
        }
        if self.view_only {
            self.counters.rejected_view_only += 1;
            return Err(InputRejectReason::ViewOnly);
        }
        Ok(())
    }

    /// Handle a pointer button press.
    pub fn on_pointer_down(
        &mut self,
        button: MouseButton,
        win_x: f64,
        win_y: f64,
        transform: Option<&CanvasTransform>,
    ) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        self.check_interactive()?;

        let mask = button.bit_mask();
        if mask == 0 && !self.capabilities.supports_extended_buttons {
            self.counters.rejected_unsupported += 1;
            return Err(InputRejectReason::Unsupported);
        }

        let Some(tx) = transform else {
            self.counters.rejected_no_geometry += 1;
            return Err(InputRejectReason::NoGeometry);
        };

        let Some((rx, ry)) = tx.map_window_point_to_remote(win_x, win_y) else {
            self.counters.rejected_outside_image += 1;
            return Err(InputRejectReason::OutsideImage);
        };

        self.focused = true;
        self.last_pointer = Some((rx, ry));
        if self.dragging_button.is_none() {
            self.dragging_button = Some(button);
        }

        let mut out = Vec::with_capacity(2);
        if (self.held_buttons & mask) == 0 {
            self.held_buttons |= mask;
            self.command_seq += 1;
            self.counters.accepted += 1;
            out.push(RemoteInput::PointerButton {
                button,
                down: true,
                x: rx,
                y: ry,
            });
        }
        Ok(out)
    }

    /// Handle pointer motion (hover or drag).
    pub fn on_pointer_move(
        &mut self,
        win_x: f64,
        win_y: f64,
        transform: Option<&CanvasTransform>,
    ) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        self.check_interactive()?;

        let Some(tx) = transform else {
            self.counters.rejected_no_geometry += 1;
            return Err(InputRejectReason::NoGeometry);
        };

        let mapped = tx.map_window_point_to_remote(win_x, win_y);
        let target = match mapped {
            Some(pt) => pt,
            None if self.dragging_button.is_some() || self.held_buttons != 0 => {
                // During active drag, clamp to edge of remote framebuffer so drag continues smoothly.
                clamp_window_point_to_remote(tx, win_x, win_y)
            }
            None => {
                self.counters.rejected_outside_image += 1;
                return Err(InputRejectReason::OutsideImage);
            }
        };

        if self.last_pointer == Some(target) {
            return Ok(Vec::new());
        }

        self.last_pointer = Some(target);
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::PointerMove {
            x: target.0,
            y: target.1,
        }])
    }

    /// Handle a pointer button release.
    ///
    /// Never drops a release for a held button even if the pointer moved outside the viewport
    /// or geometry became stale during a drag.
    pub fn on_pointer_up(
        &mut self,
        button: MouseButton,
        win_x: f64,
        win_y: f64,
        transform: Option<&CanvasTransform>,
    ) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        let mask = button.bit_mask();
        let was_held = (self.held_buttons & mask) != 0 || self.dragging_button == Some(button);
        if !was_held {
            return Ok(Vec::new());
        }

        if self.dragging_button == Some(button) {
            self.dragging_button = None;
        }
        self.held_buttons &= !mask;

        if !self.connected {
            self.counters.rejected_disconnected += 1;
            return Err(InputRejectReason::Disconnected);
        }

        // Resolve coordinate: mapped -> clamped -> last_pointer -> (0, 0)
        let (rx, ry) = transform
            .and_then(|tx| {
                tx.map_window_point_to_remote(win_x, win_y)
                    .or_else(|| Some(clamp_window_point_to_remote(tx, win_x, win_y)))
            })
            .or(self.last_pointer)
            .unwrap_or((0, 0));

        self.last_pointer = Some((rx, ry));
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::PointerButton {
            button,
            down: false,
            x: rx,
            y: ry,
        }])
    }

    /// Handle mouse wheel or trackpad scroll input.
    ///
    /// `is_pixels == true` treats `delta_x`/`delta_y` as logical pixels (scaled to 120-unit detents);
    /// `is_pixels == false` treats them as line units (1 line = 120 units).
    pub fn on_scroll(
        &mut self,
        win_x: f64,
        win_y: f64,
        delta_x: f64,
        delta_y: f64,
        is_pixels: bool,
        transform: Option<&CanvasTransform>,
    ) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        self.check_interactive()?;

        let (rx, ry) = if let Some(tx) = transform {
            tx.map_window_point_to_remote(win_x, win_y)
                .or(self.last_pointer)
                .ok_or_else(|| {
                    self.counters.rejected_outside_image += 1;
                    InputRejectReason::OutsideImage
                })?
        } else if let Some(pt) = self.last_pointer {
            pt
        } else {
            self.counters.rejected_no_geometry += 1;
            return Err(InputRejectReason::NoGeometry);
        };

        // Normalize into 120-unit wheel ticks. Standard line = 120; ~20px = 120 (6 units/px).
        let scale = if is_pixels { 6.0 } else { Self::WHEEL_DELTA };
        if delta_x.is_finite() {
            self.scroll_acc_x += delta_x * scale;
        }
        if delta_y.is_finite() {
            self.scroll_acc_y += delta_y * scale;
        }

        let emit_x = if self.capabilities.supports_horizontal_wheel {
            let whole = self.scroll_acc_x.trunc() as i32;
            self.scroll_acc_x -= whole as f64;
            whole
        } else {
            self.scroll_acc_x = 0.0;
            0
        };

        let emit_y = self.scroll_acc_y.trunc() as i32;
        self.scroll_acc_y -= emit_y as f64;

        if emit_x == 0 && emit_y == 0 {
            return Ok(Vec::new());
        }

        self.last_pointer = Some((rx, ry));
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::Scroll {
            x: rx,
            y: ry,
            horizontal: emit_x,
            vertical: emit_y,
        }])
    }

    /// Handle a physical key press, recording the exact `RemoteKey` identity by `key_id`.
    pub fn on_key_down(
        &mut self,
        key_id: impl Into<String>,
        key: RemoteKey,
        repeat: bool,
    ) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        self.check_interactive()?;

        let id = key_id.into();
        self.held_keys.insert(id, key.clone());
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::PhysicalKey {
            key,
            down: true,
            repeat,
        }])
    }

    /// Handle a physical key release, releasing the exact `RemoteKey` stored at key-down.
    pub fn on_key_up(&mut self, key_id: &str) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        let Some(key) = self.held_keys.remove(key_id) else {
            return Ok(Vec::new());
        };
        if !self.connected {
            self.counters.rejected_disconnected += 1;
            return Err(InputRejectReason::Disconnected);
        }
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::PhysicalKey {
            key,
            down: false,
            repeat: false,
        }])
    }

    /// Handle committed Unicode/IME text.
    pub fn on_committed_text(&mut self, text: &str) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        self.check_interactive()?;
        if text.is_empty() {
            return Ok(Vec::new());
        }
        if !self.capabilities.supports_unicode_text {
            self.counters.rejected_unsupported += 1;
            return Err(InputRejectReason::Unsupported);
        }
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::CommittedText(text.to_string())])
    }

    /// Handle a typed atomic shortcut request.
    pub fn on_shortcut(
        &mut self,
        shortcut: RemoteShortcut,
    ) -> Result<Vec<RemoteInput>, InputRejectReason> {
        self.counters.ui_received += 1;
        self.check_interactive()?;
        if !self.capabilities.supports_shortcuts {
            self.counters.rejected_unsupported += 1;
            return Err(InputRejectReason::Unsupported);
        }
        self.command_seq += 1;
        self.counters.accepted += 1;
        Ok(vec![RemoteInput::Shortcut(shortcut)])
    }

    /// Release all held keys, buttons, drag capture, and scroll accumulators.
    ///
    /// Emits explicit releases for every held key and button so the remote host
    /// never retains stuck modifiers or dragged mouse buttons.
    pub fn release_all(&mut self) -> Vec<RemoteInput> {
        let mut releases = Vec::new();
        let last_pt = self.last_pointer.unwrap_or((0, 0));

        for (button, mask) in [
            (MouseButton::Left, crate::input::pointer_mask::BUTTON_LEFT),
            (
                MouseButton::Middle,
                crate::input::pointer_mask::BUTTON_MIDDLE,
            ),
            (MouseButton::Right, crate::input::pointer_mask::BUTTON_RIGHT),
        ] {
            if (self.held_buttons & mask) != 0 {
                releases.push(RemoteInput::PointerButton {
                    button,
                    down: false,
                    x: last_pt.0,
                    y: last_pt.1,
                });
            }
        }
        self.held_buttons = 0;
        self.dragging_button = None;
        self.scroll_acc_x = 0.0;
        self.scroll_acc_y = 0.0;

        for (_, key) in self.held_keys.drain() {
            releases.push(RemoteInput::PhysicalKey {
                key,
                down: false,
                repeat: false,
            });
        }
        self.modifiers = KeyModifiers::default();

        if !releases.is_empty() {
            releases.push(RemoteInput::ReleaseAll);
        }
        releases
    }

    /// Begin an atomic view-only transition.
    ///
    /// - Enabling view-only immediately blocks new input locally and returns `release_all()`
    ///   while entering `EnteringViewOnly`.
    /// - Disabling view-only enters `LeavingViewOnly` and remains blocked (`view_only = true`)
    ///   until `ack_view_only_transition` confirms the matching `transition_id`.
    pub fn begin_view_only_transition(
        &mut self,
        enabled: bool,
        transition_id: u64,
    ) -> Vec<RemoteInput> {
        if enabled {
            let releases = if self.connected {
                self.release_all()
            } else {
                Vec::new()
            };
            self.view_only = true;
            self.policy_state = ViewOnlyPolicyState::EnteringViewOnly { transition_id };
            releases
        } else {
            // Fail closed until worker acknowledges
            self.view_only = true;
            self.policy_state = ViewOnlyPolicyState::LeavingViewOnly { transition_id };
            Vec::new()
        }
    }

    /// Acknowledge a view-only transition from the worker; ignores stale/mismatched IDs.
    pub fn ack_view_only_transition(&mut self, enabled: bool, transition_id: u64) -> bool {
        match self.policy_state {
            ViewOnlyPolicyState::EnteringViewOnly { transition_id: id }
                if id == transition_id && enabled =>
            {
                self.view_only = true;
                self.policy_state = ViewOnlyPolicyState::ViewOnly;
                true
            }
            ViewOnlyPolicyState::LeavingViewOnly { transition_id: id }
                if id == transition_id && !enabled =>
            {
                self.view_only = false;
                self.policy_state = ViewOnlyPolicyState::Interactive;
                true
            }
            _ if transition_id == 0 => {
                // Initial connection policy sync
                self.view_only = enabled;
                self.policy_state = if enabled {
                    ViewOnlyPolicyState::ViewOnly
                } else {
                    ViewOnlyPolicyState::Interactive
                };
                true
            }
            _ => false,
        }
    }

    /// Wrap a normalized `RemoteInput` in an epoch-bound `InputEnvelope`.
    pub fn wrap_envelope(&mut self, input: RemoteInput) -> InputEnvelope {
        self.command_seq = self.command_seq.wrapping_add(1);
        InputEnvelope {
            tab_id: self.tab_id,
            connection_generation: self.connection_generation,
            transport_epoch: self.transport_epoch,
            seq: self.command_seq,
            input,
        }
    }

    /// Transition view-only mode (legacy convenience wrapper).
    pub fn set_view_only(&mut self, enabled: bool) -> Vec<RemoteInput> {
        let releases = if enabled && !self.view_only && self.connected {
            self.release_all()
        } else {
            Vec::new()
        };
        self.view_only = enabled;
        self.policy_state = if enabled {
            ViewOnlyPolicyState::ViewOnly
        } else {
            ViewOnlyPolicyState::Interactive
        };
        releases
    }
}

fn clamp_window_point_to_remote(tx: &CanvasTransform, win_x: f64, win_y: f64) -> (u32, u32) {
    let local_x = win_x - tx.viewport_origin.0;
    let local_y = win_y - tx.viewport_origin.1;
    let rx = ((local_x - tx.image_origin.0) / tx.scale.0).floor() as i64;
    let ry = ((local_y - tx.image_origin.1) / tx.scale.1).floor() as i64;
    let max_x = tx.frame_size.0.saturating_sub(1) as i64;
    let max_y = tx.frame_size.1.saturating_sub(1) as i64;
    (rx.clamp(0, max_x) as u32, ry.clamp(0, max_y) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ScaleMode;

    fn sample_transform() -> CanvasTransform {
        CanvasTransform::new(
            (0.0, 0.0),
            (1920.0, 1080.0),
            (1920, 1080),
            ScaleMode::Fit,
            (0.0, 0.0),
        )
        .unwrap()
    }

    #[test]
    fn pointer_drag_and_release_outside_viewport_never_drops_release() {
        let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
        ctrl.connected = true;
        let tx = sample_transform();

        let down = ctrl
            .on_pointer_down(MouseButton::Left, 100.0, 200.0, Some(&tx))
            .unwrap();
        assert_eq!(
            down,
            vec![RemoteInput::PointerButton {
                button: MouseButton::Left,
                down: true,
                x: 100,
                y: 200
            }]
        );

        // Move outside viewport while dragging -> clamped edge coordinate
        let mv = ctrl.on_pointer_move(2500.0, -50.0, Some(&tx)).unwrap();
        assert_eq!(mv, vec![RemoteInput::PointerMove { x: 1919, y: 0 }]);

        // Release even when transform is None (stale geometry) -> uses last_pointer
        let up = ctrl
            .on_pointer_up(MouseButton::Left, 2500.0, -50.0, None)
            .unwrap();
        assert_eq!(
            up,
            vec![RemoteInput::PointerButton {
                button: MouseButton::Left,
                down: false,
                x: 1919,
                y: 0
            }]
        );
        assert_eq!(ctrl.held_buttons, 0);
        assert_eq!(ctrl.dragging_button, None);
    }

    #[test]
    fn key_up_releases_exact_key_identity_from_key_down() {
        let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
        ctrl.connected = true;

        let key_a = RemoteKey {
            keysym: 0x0061,
            scancode: 0x1E,
            extended: false,
            is_modifier: false,
            key_name: "a".into(),
        };
        ctrl.on_key_down("a", key_a.clone(), false).unwrap();
        let up = ctrl.on_key_up("a").unwrap();
        assert_eq!(
            up,
            vec![RemoteInput::PhysicalKey {
                key: key_a,
                down: false,
                repeat: false
            }]
        );
    }

    #[test]
    fn view_only_mid_drag_emits_release_all_then_blocks_input() {
        let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
        ctrl.connected = true;
        let tx = sample_transform();

        ctrl.on_pointer_down(MouseButton::Right, 50.0, 50.0, Some(&tx))
            .unwrap();
        let releases = ctrl.set_view_only(true);
        assert!(releases.iter().any(|r| matches!(
            r,
            RemoteInput::PointerButton {
                button: MouseButton::Right,
                down: false,
                ..
            }
        )));
        assert_eq!(
            ctrl.on_pointer_down(MouseButton::Left, 10.0, 10.0, Some(&tx)),
            Err(InputRejectReason::ViewOnly)
        );
    }

    #[test]
    fn scroll_accumulates_fractional_pixels_without_loss() {
        let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
        ctrl.connected = true;
        let tx = sample_transform();

        // 0.5 px * 6 = 3 units -> emits 3
        let s1 = ctrl
            .on_scroll(100.0, 100.0, 0.25, 0.5, true, Some(&tx))
            .unwrap();
        assert_eq!(
            s1,
            vec![RemoteInput::Scroll {
                x: 100,
                y: 100,
                horizontal: 1,
                vertical: 3
            }]
        );
        // Remaining 0.5 in x -> next 0.1 gives 0.5 + 0.6 = 1.1 -> emits 1
        let s2 = ctrl
            .on_scroll(100.0, 100.0, 0.1, 0.0, true, Some(&tx))
            .unwrap();
        assert_eq!(
            s2,
            vec![RemoteInput::Scroll {
                x: 100,
                y: 100,
                horizontal: 1,
                vertical: 0
            }]
        );
    }

    #[test]
    fn submitted_ledger_drains_non_modifiers_before_modifiers_and_view_only_is_fail_closed() {
        let mut ledger = SubmittedInputLedger::new();
        let ctrl = RemoteKey {
            keysym: 0xFFE3,
            scancode: 0x1D,
            extended: false,
            is_modifier: true,
            key_name: "control".into(),
        };
        let key_c = RemoteKey {
            keysym: 0x0063,
            scancode: 0x2E,
            extended: false,
            is_modifier: false,
            key_name: "c".into(),
        };
        ledger.record_submitted(&RemoteInput::PointerButton {
            button: MouseButton::Left,
            down: true,
            x: 10,
            y: 20,
        });
        ledger.record_submitted(&RemoteInput::PhysicalKey {
            key: ctrl.clone(),
            down: true,
            repeat: false,
        });
        ledger.record_submitted(&RemoteInput::PhysicalKey {
            key: key_c.clone(),
            down: true,
            repeat: false,
        });

        let releases = ledger.drain_releases();
        assert_eq!(releases.len(), 3);
        assert!(matches!(
            &releases[0],
            RemoteInput::PointerButton {
                button: MouseButton::Left,
                down: false,
                x: 10,
                y: 20
            }
        ));
        assert!(matches!(
            &releases[1],
            RemoteInput::PhysicalKey { key, down: false, .. } if key.key_name == "c"
        ));
        assert!(matches!(
            &releases[2],
            RemoteInput::PhysicalKey { key, down: false, .. } if key.key_name == "control"
        ));
        assert!(ledger.is_empty());

        // Atomic view-only transition: leaving view-only stays blocked until matching ack
        let mut ctrlr = RemoteInputController::new(1, true, InputCapabilities::default());
        ctrlr.connected = true;
        ctrlr.begin_view_only_transition(false, 7);
        assert!(ctrlr.view_only);
        assert!(!ctrlr.ack_view_only_transition(false, 6)); // stale ID ignored
        assert!(ctrlr.view_only);
        assert!(ctrlr.ack_view_only_transition(false, 7)); // matching ack unlocks
        assert!(!ctrlr.view_only);
    }
}
