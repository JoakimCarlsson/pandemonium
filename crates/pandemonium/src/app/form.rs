//! What the window does with the form a server or an agent is described in:
//! opening it, adding and taking out variables, and moving between its boxes.
//! What saving it means is for the page it belongs to.

use crate::app::{App, Writing};
use crate::settings::{FormField, ServerForm};

impl App {
    /// Shows `form`, with the keyboard in its first box.
    pub(super) fn open_server_form(&mut self, form: ServerForm) {
        self.server_form = Some(form);
        self.write_in(Writing::FormField(FormField::Name));
    }

    /// Lets go of the form without writing anything.
    pub(super) fn cancel_server_form(&mut self) {
        self.server_form = None;
        if matches!(self.writing, Some(Writing::FormField(_))) {
            self.writing = None;
        }
    }

    /// Adds an empty variable to the form, with the keyboard in its name.
    pub(super) fn add_form_variable(&mut self) {
        if let Some(form) = self.server_form.as_mut() {
            form.add_variable("");
            let at = form.variables.len() - 1;
            self.write_in(Writing::FormField(FormField::VariableName(at)));
        }
    }

    /// Takes the `at`-th variable out of the form.
    pub(super) fn remove_form_variable(&mut self, at: usize) {
        if let Some(form) = self.server_form.as_mut() {
            form.remove_variable(at);
        }
        if matches!(self.writing, Some(Writing::FormField(_))) {
            self.writing = None;
        }
    }

    /// Gives the keyboard to the box after `field`, or the one before it.
    pub(super) fn step_form_field(&mut self, field: FormField, backwards: bool) {
        let Some(form) = self.server_form.as_ref() else {
            return;
        };
        let fields = form.fields();
        let Some(at) = fields.iter().position(|candidate| *candidate == field) else {
            return;
        };
        let next = match backwards {
            true => (at + fields.len() - 1) % fields.len(),
            false => (at + 1) % fields.len(),
        };
        self.write_in(Writing::FormField(fields[next]));
    }
}
