//! Mobile actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_mobile_modal(&mut self, cx: &mut Context<Self>) {
        let mut lifecycle_manager = MobileLifecycleManager::new(MobilePlatform::Ios);
        lifecycle_manager.mark_connected();
        let accessory_bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);

        self.mobile_modal = Some(MobileSimulatorModalState {
            preset: MobileViewportPreset::IPhone15Pro,
            accessory_bar,
            lifecycle_manager,
            simulated_output: vec![
                "Last login: user@mobile on ttys002".into(),
                format!(
                    "Shellumi Mobile Touch Simulator v{} (iOS/Android Prototype)",
                    env!("CARGO_PKG_VERSION")
                ),
                "Virtual accessory keys stream VT/ANSI bytes into the mobile session.".into(),
                "$ ".into(),
            ],
            input_history: Vec::new(),
        });
        cx.notify();
    }

    pub(crate) fn dismiss_mobile_modal(&mut self, cx: &mut Context<Self>) {
        self.mobile_modal = None;
        cx.notify();
    }

    pub(crate) fn switch_mobile_preset(
        &mut self,
        preset: MobileViewportPreset,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut m) = self.mobile_modal {
            m.preset = preset;
            m.input_history
                .push(format!("Switched viewport to {}", preset.label()));
        }
        cx.notify();
    }

    pub(crate) fn trigger_mobile_lifecycle(
        &mut self,
        event: MobileLifecycleEvent,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut m) = self.mobile_modal {
            let label = match &event {
                MobileLifecycleEvent::AppForegrounded => "App Foregrounded",
                MobileLifecycleEvent::AppBackgrounded { .. } => "App Backgrounded (30s grace)",
                MobileLifecycleEvent::BackgroundTimerTick { .. } => "Timer Tick (10s)",
                MobileLifecycleEvent::BackgroundExpired => "Background Expired",
                MobileLifecycleEvent::NetworkLost => "Network Lost",
                MobileLifecycleEvent::NetworkRestored => "Network Restored",
                MobileLifecycleEvent::UserDisconnect => "User Disconnect",
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            m.lifecycle_manager.transition(event, now);
            m.input_history.push(format!(
                "[Lifecycle] {} -> State: {}",
                label,
                m.lifecycle_manager.state.status_label()
            ));
        }
        cx.notify();
    }

    pub(crate) fn press_mobile_virtual_key(&mut self, key: VirtualKey, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.mobile_modal {
            let bytes = m.accessory_bar.press_key(&key);
            let hex = bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let label = key.label();
            m.input_history.push(format!(
                "Key [{}] -> Bytes: [{}] Hex: {}",
                label,
                bytes
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                hex
            ));
            if let Some(last) = m.simulated_output.last_mut() {
                if key == VirtualKey::ArrowUp || key == VirtualKey::ArrowDown {
                    last.push_str(&format!(" [Key:{}]", label));
                } else if key == VirtualKey::ControlKey('c') {
                    m.simulated_output.push("^C".into());
                    m.simulated_output.push("$ ".into());
                } else if key == VirtualKey::Escape {
                    last.push_str(" <ESC>");
                } else {
                    last.push_str(&label);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_mobile_modifier(
        &mut self,
        modifier: VirtualKeyModifier,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut m) = self.mobile_modal {
            let active = m.accessory_bar.toggle_modifier(modifier);
            let mod_label = match modifier {
                VirtualKeyModifier::Ctrl => "Ctrl",
                VirtualKeyModifier::Alt => "Alt",
                VirtualKeyModifier::Shift => "Shift",
            };
            m.input_history.push(format!(
                "Sticky modifier [{}] is now {}",
                mod_label,
                if active { "ACTIVE" } else { "OFF" }
            ));
        }
        cx.notify();
    }
}
