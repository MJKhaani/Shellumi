#![allow(dead_code)]
//! Shared `TextField` and `PasswordField` primitives built on `form_field` and `TextInput`.

use crate::text_input::TextInput;
use crate::ui::components::form::form_field;
use gpui::{div, prelude::*, Div, Entity, IntoElement, SharedString};

/// Shared framed text field with label, required marker, inline error, and helper text.
pub(crate) struct TextField {
    label: SharedString,
    required: bool,
    input: Entity<TextInput>,
    error: Option<SharedString>,
    helper: Option<SharedString>,
}

impl TextField {
    pub fn new(label: impl Into<SharedString>, input: Entity<TextInput>) -> Self {
        Self {
            label: label.into(),
            required: false,
            input,
            error: None,
            helper: None,
        }
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn error(mut self, error: Option<impl Into<SharedString>>) -> Self {
        self.error = error.map(Into::into);
        self
    }

    pub fn helper(mut self, helper: Option<impl Into<SharedString>>) -> Self {
        self.helper = helper.map(Into::into);
        self
    }

    pub fn into_element_div(self) -> Div {
        form_field(
            self.label,
            self.required,
            div().w_full().child(self.input),
            self.error,
            self.helper,
        )
    }
}

impl IntoElement for TextField {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.into_element_div()
    }
}

/// Shared framed password/secret field with integrated visibility toggle and inline validation.
pub(crate) struct PasswordField {
    inner: TextField,
}

impl PasswordField {
    pub fn new(label: impl Into<SharedString>, input: Entity<TextInput>) -> Self {
        Self {
            inner: TextField::new(label, input),
        }
    }

    pub fn required(mut self, required: bool) -> Self {
        self.inner = self.inner.required(required);
        self
    }

    pub fn error(mut self, error: Option<impl Into<SharedString>>) -> Self {
        self.inner = self.inner.error(error);
        self
    }

    pub fn helper(mut self, helper: Option<impl Into<SharedString>>) -> Self {
        self.inner = self.inner.helper(helper);
        self
    }
}

impl IntoElement for PasswordField {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.inner.into_element_div()
    }
}
