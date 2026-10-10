//! Window-held inputs and render caches for worktree-owned notebooks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use pm_core::Scope;
use pm_core::notebook::{CellId, Kernel, Notebook};
use pm_ui::Scrolled;
use serde_json::{Value, json};

use crate::editor::FileId;
use crate::image::Decodes;
use crate::input::{Input, Submit};

/// An explicit notebook toolbar or cell operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Add a code cell after the named cell, or at the end.
    AddCode(Option<CellId>),
    /// Add a Markdown cell after the named cell, or at the end.
    AddMarkdown(Option<CellId>),
    /// Remove a cell.
    Delete(CellId),
    /// Move a cell one position upward.
    Up(CellId),
    /// Move a cell one position downward.
    Down(CellId),
    /// Change code to Markdown or Markdown to code.
    ChangeType(CellId),
    /// Execute one code cell.
    Run(CellId),
    /// Execute code cells in document order.
    RunAll,
    /// Clear every saved output.
    Clear,
    /// Select a discovered kernelspec.
    SelectKernel(usize),
    /// Start the selected kernel.
    Start,
    /// Interrupt current execution.
    Interrupt,
    /// Replace the kernel with a fresh one.
    Restart,
    /// Shut down the running kernel.
    Shutdown,
    /// Retry environment and kernelspec discovery.
    Refresh,
    /// Toggle between notebook and JSON editing.
    Json,
    /// Save through the ordinary file seam.
    Save,
}

/// The state of one notebook tab shared across splits.
pub struct Entry {
    /// The owning project and worktree, including when parsing fails.
    pub scope: Scope,
    /// The notebook model, or a parse error that leaves JSON editable.
    pub document: Result<Notebook, String>,
    /// Editable cell sources using the existing text input.
    pub inputs: BTreeMap<CellId, Input>,
    /// The worktree-bound protocol bridge.
    pub kernel: Option<Kernel>,
    /// Installed kernelspecs returned by the bridge.
    pub specs: Vec<Value>,
    /// Selected kernelspec name, retained even when not installed.
    pub selected: Option<String>,
    /// Last received kernel execution state.
    pub state: String,
    /// Actionable discovery, startup or execution failure.
    pub error: Option<String>,
    /// Scroll shared by views of the same document.
    pub scroll: Scrolled,
    /// Each cell rail’s scroll origin while its drag is active.
    pub scroll_origins: BTreeMap<CellId, usize>,
    /// Whether the document is being edited as JSON.
    pub raw: bool,
    /// The backing file version last synchronized with the notebook.
    pub version: i32,
}

impl Entry {
    /// Reconciles source inputs with the document after structural edits.
    pub fn reconcile(&mut self) {
        let Ok(document) = &self.document else { return };
        self.inputs
            .retain(|id, _| document.cells.iter().any(|cell| cell.id == *id));
        for cell in &document.cells {
            self.inputs.entry(cell.id).or_insert_with(|| {
                let mut input = Input::many_lines(if cell.kind() == "markdown" {
                    "cell.md"
                } else {
                    "cell.py"
                })
                .submitting(Submit::Chord);
                input.set(&cell.source());
                input
            });
        }
    }

    /// Copies edited inputs into the model and reports whether source changed.
    pub fn edited(&mut self) -> bool {
        if self.raw {
            return false;
        }
        let Ok(document) = &mut self.document else {
            return false;
        };
        let mut changed = false;
        for cell in &mut document.cells {
            if let Some(input) = self.inputs.get(&cell.id) {
                let source = input.value();
                if cell.source() != source {
                    cell.data["source"] = json!(source);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Sends a lifecycle command to the bridge, reporting a lost transport.
    pub fn send(&mut self, command: Value) {
        if let Some(kernel) = &self.kernel {
            if let Err(error) = kernel.send(command) {
                self.error = Some(error);
                self.state = "failed".into();
                self.stop_cells();
            }
        } else {
            self.error = Some("Refresh kernels to connect to Jupyter first".into());
        }
    }

    /// Takes pending kernel events without waiting for the child.
    pub fn take_events(&mut self) -> bool {
        let events = self.kernel.as_ref().map_or_else(Vec::new, Kernel::drain);
        let mut changed = false;
        for event in events {
            match event["type"].as_str().unwrap_or("") {
                "kernels" => {
                    self.specs = event["kernels"].as_array().cloned().unwrap_or_default();
                    if self.selected.is_none() {
                        self.selected = self
                            .specs
                            .first()
                            .and_then(|spec| spec["name"].as_str())
                            .map(str::to_owned);
                    }
                    self.error = self.specs.is_empty().then(|| "No Jupyter kernels found. Install ipykernel and run python -m ipykernel install --user, then refresh kernels.".into());
                    self.state = "stopped".into();
                }
                "state" => {
                    self.state = event["state"].as_str().unwrap_or("unknown").into();
                    if self.state == "idle" {
                        self.error = None;
                    }
                    if matches!(self.state.as_str(), "starting" | "stopped") {
                        self.stop_cells();
                    }
                }
                "failure" => {
                    self.state = "failed".into();
                    self.error = event["message"].as_str().map(str::to_owned);
                    self.stop_cells();
                }
                _ => {
                    if let Ok(document) = &mut self.document {
                        document.event(&event);
                        changed |= matches!(
                            event["type"].as_str(),
                            Some(
                                "execute_input"
                                    | "stream"
                                    | "error"
                                    | "display_data"
                                    | "execute_result"
                                    | "update_display_data"
                                    | "clear_output"
                            )
                        );
                    }
                }
            }
        }
        changed
    }

    /// Marks every cell as no longer queued after a lifecycle transition.
    pub fn stop_cells(&mut self) {
        if let Ok(document) = &mut self.document {
            for cell in &mut document.cells {
                cell.running = false;
            }
        }
    }

    /// Whether execution or startup currently prevents structural cell edits.
    pub fn running(&self) -> bool {
        matches!(self.state.as_str(), "starting" | "stopping")
            || self
                .document
                .as_ref()
                .is_ok_and(|document| document.cells.iter().any(|cell| cell.running))
    }
}

/// Notebook state keyed by the existing project-scoped file identities.
#[derive(Default)]
pub struct Notebooks {
    /// Every notebook held by the pane tree.
    pub open: BTreeMap<FileId, Entry>,
    /// Rich output pictures decoded on the shared image worker pool.
    pub pictures: Decodes<(FileId, String, String)>,
}

impl Notebooks {
    /// Whether a file should be opened as a notebook document.
    pub fn is_notebook(path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ipynb"))
    }

    /// Opens a notebook model without starting a kernel or running a cell.
    pub fn open(
        &mut self,
        file: FileId,
        scope: Scope,
        source: &str,
        version: i32,
        root: &Path,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) {
        let document = Notebook::parse(scope, source);
        let selected = document
            .as_ref()
            .ok()
            .and_then(|document| document.kernel_name())
            .map(str::to_owned);
        let kernel = document
            .as_ref()
            .ok()
            .map(|_| Kernel::discover(scope, root, wake));
        let mut entry = Entry {
            scope,
            document,
            inputs: BTreeMap::new(),
            kernel,
            specs: Vec::new(),
            selected,
            state: "discovering".into(),
            error: None,
            scroll: Scrolled::default(),
            scroll_origins: BTreeMap::new(),
            raw: false,
            version,
        };
        entry.reconcile();
        self.open.insert(file, entry);
    }

    /// Closes every notebook and output cache whose owning scope is removed.
    pub fn forget(&mut self, removed: impl Fn(Scope) -> bool) {
        self.open.retain(|_, entry| !removed(entry.scope));
        self.pictures
            .retain(|(file, _, _)| self.open.contains_key(file));
    }

    /// Closes bridges and image caches when the last tab releases a notebook.
    pub fn retain(&mut self, files: &BTreeSet<FileId>) {
        self.open.retain(|file, _| files.contains(file));
        self.pictures.retain(|(file, _, _)| files.contains(file));
    }
}
