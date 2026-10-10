//! Notebook JSON with stable cell identities and preserved unknown data.

use serde_json::{Value, json};

use crate::Scope;

/// A cell identity that survives movement within a document.
pub type CellId = u64;

/// One cell, including metadata and MIME bundles the editor does not understand.
pub struct Cell {
    /// The identity used for editor inputs and execution requests.
    pub id: CellId,
    /// The original JSON, updated only by explicit edits and execution.
    pub data: Value,
    /// Whether this cell is queued or executing.
    pub running: bool,
    /// Whether the next output should apply a deferred clear.
    clear_pending: bool,
}

impl Cell {
    /// Returns the concatenated source regardless of its on-disk representation.
    pub fn source(&self) -> String {
        multiline(&self.data["source"])
    }

    /// Returns the notebook cell type, including unknown types.
    pub fn kind(&self) -> &str {
        self.data["cell_type"].as_str().unwrap_or("unknown")
    }

    /// Returns existing outputs without narrowing their schema.
    pub fn outputs(&self) -> &[Value] {
        self.data["outputs"].as_array().map_or(&[], Vec::as_slice)
    }

    /// Clears outputs and execution count without disturbing cell metadata.
    pub fn clear(&mut self) {
        if self.kind() == "code" {
            self.data["outputs"] = json!([]);
            self.data["execution_count"] = Value::Null;
        }
        self.clear_pending = false;
    }

    /// Appends an output after applying any deferred clear request.
    fn output(&mut self, output: Value) {
        if self.clear_pending {
            self.data["outputs"] = json!([]);
            self.clear_pending = false;
        }
        if let Some(outputs) = self.data["outputs"].as_array_mut() {
            outputs.push(output);
        }
    }
}

/// One notebook belonging to exactly one project and worktree.
pub struct Notebook {
    /// The document's owning project and worktree.
    pub scope: Scope,
    /// Cells in their document order.
    pub cells: Vec<Cell>,
    /// Top-level fields, including all notebook metadata.
    data: Value,
    /// The next stable editor identity.
    next: CellId,
}

impl Notebook {
    /// Parses a version-four notebook without executing any source.
    pub fn parse(scope: Scope, source: &str) -> Result<Self, String> {
        let mut data: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
        if data["nbformat"] != 4 || !data["metadata"].is_object() {
            return Err(
                "Expected a version 4 notebook with metadata; edit the JSON to repair it".into(),
            );
        }
        let Value::Array(cells) = data["cells"].take() else {
            return Err("Notebook cells must be an array".into());
        };
        let mut parsed = Vec::new();
        for (index, cell) in cells.into_iter().enumerate() {
            if !cell.is_object()
                || !cell["metadata"].is_object()
                || !cell["cell_type"].is_string()
                || !(cell["source"].is_string()
                    || cell["source"]
                        .as_array()
                        .is_some_and(|lines| lines.iter().all(Value::is_string)))
                || (cell["cell_type"] == "code" && !cell["outputs"].is_array())
            {
                return Err(format!(
                    "Invalid cell {}; edit the JSON to repair it",
                    index + 1
                ));
            }
            parsed.push(Cell {
                id: index as u64,
                data: cell,
                running: false,
                clear_pending: false,
            });
        }
        let next = parsed.len() as u64;
        Ok(Self {
            scope,
            cells: parsed,
            data,
            next,
        })
    }

    /// Serializes the notebook while retaining unedited fields and unsupported outputs.
    pub fn serialize(&self) -> String {
        let mut data = self.data.clone();
        data["cells"] = self.cells.iter().map(|cell| cell.data.clone()).collect();
        format!(
            "{}\n",
            serde_json::to_string_pretty(&data).expect("JSON values serialize")
        )
    }

    /// Returns the cell named by a stable editor identity.
    pub fn cell_mut(&mut self, id: CellId) -> Option<&mut Cell> {
        self.cells.iter_mut().find(|cell| cell.id == id)
    }

    /// Inserts an empty cell after the requested cell, or at the end.
    pub fn add(&mut self, after: Option<CellId>, markdown: bool) -> CellId {
        let id = self.next;
        self.next += 1;
        let mut serial = format!("pandemonium-{id}");
        while self.cells.iter().any(|cell| cell.data["id"] == serial) {
            serial.push('-');
        }
        let mut data = json!({"id": serial, "cell_type": if markdown { "markdown" } else { "code" }, "source": [], "metadata": {}});
        if !markdown {
            data["outputs"] = json!([]);
            data["execution_count"] = Value::Null;
        }
        let index = after
            .and_then(|id| self.cells.iter().position(|cell| cell.id == id))
            .map_or(self.cells.len(), |index| index + 1);
        self.cells.insert(
            index,
            Cell {
                id,
                data,
                running: false,
                clear_pending: false,
            },
        );
        id
    }

    /// Changes a cell's type while retaining source and metadata.
    pub fn change_type(&mut self, id: CellId) {
        if let Some(cell) = self.cell_mut(id) {
            let code = cell.kind() != "code";
            cell.data["cell_type"] = json!(if code { "code" } else { "markdown" });
            if code {
                if let Some(attachments) = cell
                    .data
                    .as_object_mut()
                    .and_then(|data| data.remove("attachments"))
                {
                    cell.data["metadata"]["pandemonium_attachments"] = attachments;
                }
                cell.clear();
            } else if let Some(data) = cell.data.as_object_mut() {
                data.remove("outputs");
                data.remove("execution_count");
                if let Some(attachments) = data
                    .get_mut("metadata")
                    .and_then(Value::as_object_mut)
                    .and_then(|data| data.remove("pandemonium_attachments"))
                {
                    data.insert("attachments".into(), attachments);
                }
            }
        }
    }

    /// Moves a cell one place, staying within the document.
    pub fn move_cell(&mut self, id: CellId, down: bool) {
        if let Some(index) = self.cells.iter().position(|cell| cell.id == id) {
            let destination = if down {
                (index + 1).min(self.cells.len() - 1)
            } else {
                index.saturating_sub(1)
            };
            self.cells.swap(index, destination);
        }
    }

    /// Records the selected kernelspec without removing unrelated kernel metadata.
    pub fn select_kernel(&mut self, name: &str, display: &str, language: &str) {
        if !self.data["metadata"]["kernelspec"].is_object() {
            self.data["metadata"]["kernelspec"] = json!({});
        }
        let spec = &mut self.data["metadata"]["kernelspec"];
        spec["name"] = json!(name);
        spec["display_name"] = json!(display);
        spec["language"] = json!(language);
    }

    /// Returns the saved kernelspec name when one is present.
    pub fn kernel_name(&self) -> Option<&str> {
        self.data["metadata"]["kernelspec"]["name"].as_str()
    }

    /// Applies a Jupyter event, preserving rich output data and display updates.
    pub fn event(&mut self, event: &Value) {
        let kind = event["type"].as_str().unwrap_or("");
        if kind == "update_display_data" {
            let display = &event["content"]["transient"]["display_id"];
            if display.is_string() {
                for cell in &mut self.cells {
                    if let Some(outputs) =
                        cell.data.get_mut("outputs").and_then(Value::as_array_mut)
                    {
                        for output in outputs {
                            if output["metadata"]["pandemonium_display_id"] == *display {
                                output["data"] = event["content"]["data"].clone();
                                let mut metadata = event["content"]["metadata"].clone();
                                metadata["pandemonium_display_id"] = display.clone();
                                output["metadata"] = metadata;
                            }
                        }
                    }
                }
            }
            return;
        }
        let Some(id) = event["cell"].as_u64() else {
            return;
        };
        let Some(cell) = self.cell_mut(id).filter(|cell| cell.kind() == "code") else {
            return;
        };
        let content = &event["content"];
        match kind {
            "execute_input" => cell.data["execution_count"] = content["execution_count"].clone(),
            "status" => cell.running = content["execution_state"] != "idle",
            "done" => cell.running = false,
            "clear_output" => {
                if content["wait"] == true {
                    cell.clear_pending = true;
                } else {
                    cell.data["outputs"] = json!([]);
                    cell.clear_pending = false;
                }
            }
            "stream" => cell.output(json!({"output_type": "stream", "name": content["name"], "text": content["text"]})),
            "error" => cell.output(json!({"output_type": "error", "ename": content["ename"], "evalue": content["evalue"], "traceback": content["traceback"]})),
            "display_data" | "execute_result" => {
                let mut output = json!({"output_type": kind, "data": content["data"], "metadata": content["metadata"]});
                if let Some(id) = content["transient"]["display_id"].as_str() {
                    output["metadata"]["pandemonium_display_id"] = json!(id);
                }
                if kind == "execute_result" {
                    output["execution_count"] = content["execution_count"].clone();
                }
                cell.output(output);
            }
            _ => {}
        }
    }
}

/// Reads notebook multiline strings in both permitted representations.
pub fn multiline(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(lines) => lines.iter().filter_map(Value::as_str).collect(),
        _ => String::new(),
    }
}
