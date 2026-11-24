#![allow(dead_code, unused_imports)]
//! Unified Inspector state machine, coordinator, and typed drafts.

pub mod coordinator;
pub mod drafts;

pub use coordinator::{
    AsyncOperationState, InspectorCoordinator, InspectorMode, InspectorState, InspectorTarget,
    NavigationRequest,
};
pub use drafts::*;
