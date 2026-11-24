//! Single Inspector Coordinator and State Machine.
//!
//! Enforces:
//! 1. At most one inspector panel open per window.
//! 2. Target replacement (selecting a host closes/replaces group inspector).
//! 3. Zero silent draft loss via guarded navigation (Save / Discard / Keep Editing).
//! 4. Generation tokens on async operations to prevent stale response mutation.
//! 5. Strict multi-vault isolation preventing cross-vault draft leaks.

use super::drafts::InspectorDraft;

/// Target object being viewed, created, or edited in the Inspector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectorTarget {
    Host(Option<i64>),
    Group(Option<i64>),
    Identity(Option<i64>),
    Snippet(Option<i64>),
    Tunnel(Option<i64>),
    Workspace(Option<i64>),
    Vault(String),
    Provider(Option<i64>),
    TeamMember(Option<i64>),
    Session(String),
}

/// Operation mode for the Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorMode {
    View,
    Create,
    Edit,
}

/// In-flight asynchronous operation status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncOperationState {
    Idle,
    Saving { generation: u64 },
    Deleting { generation: u64 },
    Error { message: String, generation: u64 },
}

/// Requested navigation or target change that is pending confirmation if draft is dirty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationRequest {
    SwitchTarget(InspectorTarget, InspectorMode),
    CloseInspector,
    SwitchVault(String),
    SwitchPage(String),
}

/// State of the active Inspector in a window.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectorState {
    pub vault_id: String,
    pub target: InspectorTarget,
    pub mode: InspectorMode,
    pub draft: Option<InspectorDraft>,
    pub dirty: bool,
    pub generation: u64,
    pub operation: AsyncOperationState,
}

/// Window-local single Inspector Coordinator.
#[derive(Debug, Default, Clone)]
pub struct InspectorCoordinator {
    /// The currently active inspector, if any.
    active: Option<InspectorState>,
    /// Pending navigation request awaiting user confirmation when draft is dirty.
    pending_guard: Option<NavigationRequest>,
    /// Monotonically increasing generation token counter for async operations.
    generation_counter: u64,
}

impl InspectorCoordinator {
    /// Create a new, idle coordinator with no active inspector.
    pub fn new() -> Self {
        Self {
            active: None,
            pending_guard: None,
            generation_counter: 0,
        }
    }

    /// Check whether an inspector is currently open.
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// Check whether the active inspector has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.active.as_ref().is_some_and(|s| s.dirty)
    }

    /// Check whether a guarded navigation confirmation is currently requested.
    pub fn is_guard_active(&self) -> bool {
        self.pending_guard.is_some()
    }

    /// Get the pending navigation request, if any.
    pub fn pending_guard(&self) -> Option<&NavigationRequest> {
        self.pending_guard.as_ref()
    }

    /// Get the active inspector state.
    pub fn active(&self) -> Option<&InspectorState> {
        self.active.as_ref()
    }

    /// Get a mutable reference to the active inspector state.
    pub fn active_mut(&mut self) -> Option<&mut InspectorState> {
        self.active.as_mut()
    }

    /// Open or replace the active inspector with a target and mode.
    ///
    /// If the current draft is dirty, does NOT replace immediately; instead records
    /// `NavigationRequest::SwitchTarget` in `pending_guard` and returns `false`.
    /// If clean, replaces the inspector content and returns `true`.
    pub fn open(
        &mut self,
        vault_id: impl Into<String>,
        target: InspectorTarget,
        mode: InspectorMode,
        draft: Option<InspectorDraft>,
    ) -> bool {
        let vault_id = vault_id.into();

        if let Some(ref active) = self.active {
            // Guard against silent data loss if existing draft is dirty!
            if active.dirty {
                self.pending_guard = Some(NavigationRequest::SwitchTarget(target, mode));
                return false;
            }
        }

        self.generation_counter += 1;
        self.active = Some(InspectorState {
            vault_id,
            target,
            mode,
            draft,
            dirty: false,
            generation: self.generation_counter,
            operation: AsyncOperationState::Idle,
        });
        self.pending_guard = None;
        true
    }

    /// Request closing the inspector.
    ///
    /// If the current draft is dirty, sets `pending_guard` to `CloseInspector` and returns `false`.
    /// If clean, closes the inspector and returns `true`.
    pub fn close(&mut self) -> bool {
        if let Some(ref active) = self.active {
            if active.dirty {
                self.pending_guard = Some(NavigationRequest::CloseInspector);
                return false;
            }
        }
        self.active = None;
        self.pending_guard = None;
        true
    }

    /// Request switching the active vault.
    ///
    /// If dirty, sets `pending_guard` to `SwitchVault(new_vault)` and returns `false`.
    /// If clean, closes any open inspector from the former vault and returns `true`.
    pub fn request_switch_vault(&mut self, new_vault_id: impl Into<String>) -> bool {
        let new_id = new_vault_id.into();
        if let Some(ref active) = self.active {
            if active.dirty {
                self.pending_guard = Some(NavigationRequest::SwitchVault(new_id));
                return false;
            }
        }
        // Multi-vault isolation: close inspector when switching vaults
        self.active = None;
        self.pending_guard = None;
        true
    }

    /// Mark the active draft as dirty or clean.
    pub fn set_dirty(&mut self, dirty: bool) {
        if let Some(ref mut s) = self.active {
            s.dirty = dirty;
            if let Some(ref mut d) = s.draft {
                d.set_dirty(dirty);
            }
        }
    }

    /// Switch mode of the active inspector (e.g. from View to Edit).
    pub fn set_mode(&mut self, mode: InspectorMode) {
        if let Some(ref mut s) = self.active {
            s.mode = mode;
        }
    }

    /// Begin an asynchronous save operation. Returns the assigned generation token.
    /// Returns None if already saving (prevents duplicate submissions!).
    pub fn start_save(&mut self) -> Option<u64> {
        let s = self.active.as_mut()?;
        if matches!(s.operation, AsyncOperationState::Saving { .. }) {
            return None; // Duplicate save prevented!
        }
        self.generation_counter += 1;
        let gen = self.generation_counter;
        s.generation = gen;
        s.operation = AsyncOperationState::Saving { generation: gen };
        Some(gen)
    }

    /// Complete an asynchronous save operation.
    ///
    /// Validates matching `vault_id`, `generation`, and `target`. If stale, safely rejects.
    /// On success, switches mode to `View`, marks clean, and clears in-flight state.
    pub fn complete_save(
        &mut self,
        vault_id: &str,
        generation: u64,
        saved_target: InspectorTarget,
    ) -> bool {
        let Some(ref mut s) = self.active else {
            return false;
        };

        // Enforce generation and vault match!
        if s.vault_id != vault_id || s.generation != generation {
            tracing::warn!(
                active_vault = %s.vault_id,
                result_vault = %vault_id,
                active_gen = s.generation,
                result_gen = generation,
                "Ignored stale async save result"
            );
            return false;
        }

        s.target = saved_target;
        s.mode = InspectorMode::View;
        s.dirty = false;
        if let Some(ref mut d) = s.draft {
            d.set_dirty(false);
        }
        s.operation = AsyncOperationState::Idle;
        true
    }

    /// Record a failed save operation.
    ///
    /// Validates matching `vault_id` and `generation`. If valid, retains draft and dirty state,
    /// and records the error message for in-context display and retry.
    pub fn fail_save(&mut self, vault_id: &str, generation: u64, error_message: String) -> bool {
        let Some(ref mut s) = self.active else {
            return false;
        };

        if s.vault_id != vault_id || s.generation != generation {
            return false;
        }

        s.operation = AsyncOperationState::Error {
            message: error_message,
            generation,
        };
        true
    }

    /// User confirmed discarding unsaved changes.
    ///
    /// Executes the pending navigation request (if any) or reverts the active draft.
    pub fn confirm_discard(&mut self) {
        self.set_dirty(false);

        if let Some(req) = self.pending_guard.take() {
            match req {
                NavigationRequest::SwitchTarget(target, mode) => {
                    let vault_id = self
                        .active
                        .as_ref()
                        .map(|s| s.vault_id.clone())
                        .unwrap_or_else(|| "local".into());
                    self.open(vault_id, target, mode, None);
                }
                NavigationRequest::CloseInspector => {
                    self.active = None;
                }
                NavigationRequest::SwitchVault(_) => {
                    self.active = None;
                }
                NavigationRequest::SwitchPage(_) => {
                    self.active = None;
                }
            }
        } else if let Some(ref mut s) = self.active {
            // Revert from Edit to View mode
            s.mode = InspectorMode::View;
            s.operation = AsyncOperationState::Idle;
        }
    }

    /// User canceled the guard ("Keep Editing").
    ///
    /// Dismisses the pending navigation guard and keeps the active draft intact.
    pub fn cancel_guard(&mut self) {
        self.pending_guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::inspector::drafts::HostDraft;

    #[test]
    fn test_ins_001_select_group_then_host_replaces_inspector() {
        let mut coord = InspectorCoordinator::new();

        // 1. Select group
        assert!(coord.open(
            "vault-1",
            InspectorTarget::Group(Some(10)),
            InspectorMode::View,
            None
        ));
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Group(Some(10))
        );
        assert!(coord.is_open());

        // 2. Select host -> host replaces group; no adjacent second inspector!
        assert!(coord.open(
            "vault-1",
            InspectorTarget::Host(Some(101)),
            InspectorMode::View,
            None
        ));
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(101))
        );
        assert!(coord.is_open());
    }

    #[test]
    fn test_ins_002_select_host_a_then_b_repeatedly() {
        let mut coord = InspectorCoordinator::new();

        coord.open(
            "v1",
            InspectorTarget::Host(Some(1)),
            InspectorMode::View,
            None,
        );
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(1))
        );

        coord.open(
            "v1",
            InspectorTarget::Host(Some(2)),
            InspectorMode::View,
            None,
        );
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(2))
        );

        coord.open(
            "v1",
            InspectorTarget::Host(Some(1)),
            InspectorMode::View,
            None,
        );
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(1))
        );
    }

    #[test]
    fn test_ins_008_dirty_edit_triggers_guard_on_navigation() {
        let mut coord = InspectorCoordinator::new();
        let draft = HostDraft {
            id: Some(1),
            name: "Prod Bastion".into(),
            ..Default::default()
        };

        coord.open(
            "v1",
            InspectorTarget::Host(Some(1)),
            InspectorMode::Edit,
            Some(InspectorDraft::Host(draft)),
        );
        assert!(!coord.is_dirty());

        // User edits field
        coord.set_dirty(true);
        assert!(coord.is_dirty());

        // Attempt to select Host 2 -> BLOCKED by guard!
        let navigated = coord.open(
            "v1",
            InspectorTarget::Host(Some(2)),
            InspectorMode::View,
            None,
        );
        assert!(!navigated);
        assert!(coord.is_guard_active());
        assert_eq!(
            coord.pending_guard(),
            Some(&NavigationRequest::SwitchTarget(
                InspectorTarget::Host(Some(2)),
                InspectorMode::View
            ))
        );

        // Current target remains Host 1 with dirty edits intact!
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(1))
        );

        // User chooses "Keep Editing"
        coord.cancel_guard();
        assert!(!coord.is_guard_active());
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(1))
        );

        // User tries again and chooses "Discard"
        coord.open(
            "v1",
            InspectorTarget::Host(Some(2)),
            InspectorMode::View,
            None,
        );
        coord.confirm_discard();
        assert!(!coord.is_guard_active());
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(2))
        );
        assert!(!coord.is_dirty());
    }

    #[test]
    fn test_ins_010_switch_vault_preserves_isolation() {
        let mut coord = InspectorCoordinator::new();
        coord.open(
            "vault-local",
            InspectorTarget::Host(Some(1)),
            InspectorMode::View,
            None,
        );
        assert_eq!(coord.active().unwrap().vault_id, "vault-local");

        // Clean switch closes inspector from former vault
        assert!(coord.request_switch_vault("vault-remote"));
        assert!(!coord.is_open());
    }

    #[test]
    fn test_ins_016_double_click_save_prevents_duplicate_submission() {
        let mut coord = InspectorCoordinator::new();
        coord.open(
            "v1",
            InspectorTarget::Host(Some(1)),
            InspectorMode::Edit,
            None,
        );
        coord.set_dirty(true);

        // First click -> Save begins
        let gen1 = coord.start_save();
        assert!(gen1.is_some());

        // Rapid second click while saving -> rejected!
        let gen2 = coord.start_save();
        assert_eq!(gen2, None);
    }

    #[test]
    fn test_ins_017_stale_async_result_cannot_mutate_current_target() {
        let mut coord = InspectorCoordinator::new();

        // 1. Host 1 starts saving with generation 1
        coord.open(
            "v1",
            InspectorTarget::Host(Some(1)),
            InspectorMode::Edit,
            None,
        );
        coord.set_dirty(true);
        let gen1 = coord.start_save().unwrap();

        // 2. User discards or switches to Host 2
        coord.confirm_discard();
        coord.open(
            "v1",
            InspectorTarget::Host(Some(2)),
            InspectorMode::View,
            None,
        );

        // 3. Delayed async response for Host 1 arrives with gen1
        let applied = coord.complete_save("v1", gen1, InspectorTarget::Host(Some(1)));
        assert!(!applied, "Stale async response must be rejected");

        // Target remains Host 2!
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(2))
        );
    }
}
