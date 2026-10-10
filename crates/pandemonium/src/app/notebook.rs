//! Wiring notebook documents, cell inputs and asynchronous kernel wakes.

use pm_core::notebook::Kernel;
use serde_json::json;

use crate::app::{App, Wake, Writing};
use crate::editor::FileId;
use crate::message::Message;
use crate::notebook::{Action, Notebooks};
use crate::panes::Item;

impl App {
    /// Opens notebook models for held files and synchronizes all edited cell sources.
    pub(super) fn sync_notebooks(&mut self) {
        let files = self
            .panes
            .held()
            .iter()
            .copied()
            .filter_map(Item::file)
            .collect::<Vec<_>>();
        for file in files {
            self.sync_notebook(file);
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            let buffer = document.borrow();
            let buffer = buffer.buffer();
            if !Notebooks::is_notebook(buffer.path()) {
                continue;
            }
            let Some(scope) = self.editor.scope_of(file) else {
                continue;
            };
            let Some(root) = self.root_of(scope) else {
                continue;
            };
            let replace = self
                .notebooks
                .open
                .get(&file)
                .is_none_or(|entry| !entry.raw && entry.version != buffer.version());
            if replace {
                self.notebooks.open(
                    file,
                    scope,
                    &buffer.contents(),
                    buffer.version(),
                    &root,
                    self.waker(Wake::Notebook),
                );
                if matches!(self.writing, Some(Writing::Notebook(held, _)) if held == file) {
                    self.writing = None;
                }
            }
        }
    }

    /// Copies cell edits into the ordinary file buffer so save and dirty-close stay shared.
    pub(super) fn sync_notebook(&mut self, file: FileId) {
        let changed = self
            .notebooks
            .open
            .get_mut(&file)
            .is_some_and(|entry| entry.edited());
        if changed {
            self.write_notebook_buffer(file);
        }
    }

    /// Replaces backing JSON after a notebook edit and records its synchronized version.
    fn write_notebook_buffer(&mut self, file: FileId) {
        let Some(entry) = self.notebooks.open.get_mut(&file) else {
            return;
        };
        let Ok(document) = &entry.document else {
            return;
        };
        let source = document.serialize();
        self.editor.edit(file, |buffer| {
            if buffer.contents() != source {
                buffer.commit();
                buffer.set_contents(&source);
                buffer.commit();
            }
        });
        if let Some(document) = self.editor.get(file) {
            entry.version = document.borrow().buffer().version();
        }
    }

    /// Applies ready kernel events and writes new outputs into the shared file buffer.
    pub(super) fn take_notebook_events(&mut self) {
        let files = self
            .notebooks
            .open
            .iter_mut()
            .filter_map(|(file, entry)| entry.take_events().then_some(*file))
            .collect::<Vec<_>>();
        for file in files {
            self.write_notebook_buffer(file);
        }
        self.request_redraw();
    }

    /// Whether a file is currently drawn as a notebook rather than raw JSON.
    pub(super) fn notebook_visible(&self, file: FileId) -> bool {
        self.notebooks
            .open
            .get(&file)
            .is_some_and(|entry| !entry.raw)
    }

    /// Scrolls the notebook under the pointer through its ordinary pane scroll area.
    pub(super) fn scroll_notebook(&mut self, delta: f32) -> bool {
        let Some(Item::File(file)) = self.item_under() else {
            return false;
        };
        let Some(entry) = self.notebooks.open.get(&file).filter(|entry| !entry.raw) else {
            return false;
        };
        if let Some(pointer) = self.pointer
            && let Some(input) = entry.inputs.values().find(|input| input.covers(pointer))
        {
            input.scroll_by(-delta);
        } else {
            let mut scroll = entry.scroll.get();
            scroll.by(delta);
            entry.scroll.set(scroll);
        }
        true
    }

    /// Handles notebook controls and source selection through the shared input routing.
    pub(super) fn notebook_command(&mut self, message: Message) -> bool {
        match message {
            Message::PointNotebook(pane, file, cell, phase, anchor, head) => {
                if phase == pm_ui::ResizePhase::Started {
                    self.focus_pane(pane);
                }
                self.point_in(Writing::Notebook(file, cell), phase, anchor, head);
            }
            Message::ScrollNotebookCell(file, cell, event, step) => {
                if let Some(entry) = self.notebooks.open.get_mut(&file)
                    && let Some(input) = entry.inputs.get(&cell)
                {
                    let base = if event.phase == pm_ui::ResizePhase::Started {
                        input.rows_above()
                    } else {
                        entry
                            .scroll_origins
                            .get(&cell)
                            .copied()
                            .unwrap_or_else(|| input.rows_above())
                    };
                    if event.phase == pm_ui::ResizePhase::Ended {
                        entry.scroll_origins.remove(&cell);
                    } else {
                        entry.scroll_origins.insert(cell, base);
                    }
                    let rows = base as f32 + event.delta(pm_ui::Axis::Vertical) * step;
                    input.scroll_to_row(rows.round().max(0.0) as usize);
                }
            }
            Message::Notebook(file, action) => self.act_on_notebook(file, action),
            _ => return false,
        }
        self.request_redraw();
        true
    }

    /// Applies one explicit notebook edit or kernel command in its owning worktree.
    fn act_on_notebook(&mut self, file: FileId, action: Action) {
        self.sync_notebook(file);
        if action == Action::Save {
            if let Some(root) = self.worktree_of(file) {
                self.editor.save(file, &root);
                self.reread_changes();
            }
            return;
        }
        let Some(scope) = self.editor.scope_of(file) else {
            return;
        };
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let wake = self.waker(Wake::Notebook);
        let Some(entry) = self.notebooks.open.get_mut(&file) else {
            return;
        };
        if matches!(
            action,
            Action::AddCode(_)
                | Action::AddMarkdown(_)
                | Action::Delete(_)
                | Action::Up(_)
                | Action::Down(_)
                | Action::ChangeType(_)
                | Action::Run(_)
                | Action::RunAll
                | Action::Json
                | Action::SelectKernel(_)
        ) && entry.running()
        {
            entry.error = Some("Interrupt execution or wait for it to finish before changing cells or starting another run.".into());
            return;
        }
        if action == Action::Json {
            entry.raw = !entry.raw;
            if !entry.raw
                && let Some(document) = self.editor.get(file)
            {
                let document = document.borrow();
                let buffer = document.buffer();
                self.notebooks.open(
                    file,
                    scope,
                    &buffer.contents(),
                    buffer.version(),
                    &root,
                    wake,
                );
            }
            self.writing = None;
            return;
        }
        if action == Action::Refresh {
            entry.kernel = Some(Kernel::discover(scope, &root, wake));
            entry.state = "discovering".into();
            entry.error = None;
            entry.stop_cells();
            return;
        }
        let mut changed = false;
        match action {
            Action::Start | Action::Restart => {
                if entry.state == "starting" {
                    entry.error =
                        Some("The kernel is still starting. Wait for it to become idle.".into());
                    return;
                }
                if action == Action::Start && matches!(entry.state.as_str(), "idle" | "busy") {
                    entry.error = Some(
                        "The kernel is already running. Use Restart for a fresh namespace.".into(),
                    );
                    return;
                }
                let Some(name) = entry.selected.clone() else {
                    entry.error = Some("Select an installed kernel first, or refresh kernels after installing ipykernel.".into());
                    return;
                };
                if !entry.specs.iter().any(|spec| spec["name"] == name) {
                    entry.error = Some(format!(
                        "Kernel '{name}' is unavailable. Select an installed kernel or refresh kernels."
                    ));
                    return;
                }
                entry.stop_cells();
                entry.state = "starting".into();
                entry.error = None;
                entry.send(json!({"action": if action == Action::Start { "start" } else { "restart" }, "name": name}));
            }
            Action::Shutdown => {
                entry.stop_cells();
                entry.state = "stopping".into();
                entry.send(json!({"action": "shutdown"}));
            }
            Action::Interrupt => entry.send(json!({"action": "interrupt"})),
            Action::SelectKernel(index) => {
                if let Some(spec) = entry.specs.get(index).cloned() {
                    let name = spec["name"].as_str().unwrap_or("");
                    if entry.selected.as_deref() != Some(name) {
                        entry.state = "stopping".into();
                        entry.send(json!({"action": "shutdown"}));
                    }
                    entry.selected = Some(name.to_owned());
                    if let Ok(document) = &mut entry.document {
                        document.select_kernel(
                            name,
                            spec["display_name"].as_str().unwrap_or(name),
                            spec["language"].as_str().unwrap_or(""),
                        );
                        changed = true;
                    }
                }
            }
            Action::Run(_) | Action::RunAll => {
                if entry.state != "idle" {
                    entry.error = Some(
                        "Start the selected kernel and wait until it is idle before executing."
                            .into(),
                    );
                    return;
                }
                let Ok(document) = &mut entry.document else {
                    return;
                };
                let cells = document
                    .cells
                    .iter_mut()
                    .filter(|cell| {
                        cell.kind() == "code"
                            && match action {
                                Action::Run(id) => cell.id == id,
                                _ => true,
                            }
                    })
                    .map(|cell| {
                        cell.clear();
                        cell.running = true;
                        json!({"id": cell.id, "source": cell.source()})
                    })
                    .collect::<Vec<_>>();
                if !cells.is_empty() {
                    entry.error = None;
                    entry.send(json!({"action": "execute", "cells": cells}));
                    changed = true;
                }
            }
            _ => {
                let Ok(document) = &mut entry.document else {
                    return;
                };
                match action {
                    Action::AddCode(after) | Action::AddMarkdown(after) => {
                        document.add(after, matches!(action, Action::AddMarkdown(_)));
                    }
                    Action::Delete(id) => {
                        document.cells.retain(|cell| cell.id != id);
                    }
                    Action::Up(id) | Action::Down(id) => {
                        document.move_cell(id, matches!(action, Action::Down(_)))
                    }
                    Action::ChangeType(id) => document.change_type(id),
                    Action::Clear => {
                        for cell in &mut document.cells {
                            cell.clear();
                        }
                    }
                    _ => return,
                }
                entry.reconcile();
                if let Some(Writing::Notebook(held, id)) = self.writing
                    && held == file
                    && !entry.inputs.contains_key(&id)
                {
                    self.writing = None;
                }
                changed = true;
            }
        }
        if changed {
            self.write_notebook_buffer(file);
        }
    }
}
