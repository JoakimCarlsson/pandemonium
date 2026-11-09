//! Reading the symbols a language server names in a file or finds in a project.

use std::path::Path;

use pm_buffer::{Position, Span};
use serde_json::Value;

use crate::types::Outline;
use crate::uri::path_of;

/// The name of the symbol kind `number`, as the language server protocol numbers them, in lower case.
pub(crate) fn kind_name(number: Option<u64>) -> &'static str {
    match number {
        Some(1) => "file",
        Some(2) => "module",
        Some(3) => "namespace",
        Some(4) => "package",
        Some(5) => "class",
        Some(6) => "method",
        Some(7) => "property",
        Some(8) => "field",
        Some(9) => "constructor",
        Some(10) => "enum",
        Some(11) => "interface",
        Some(12) => "function",
        Some(13) => "variable",
        Some(14) => "constant",
        Some(15) => "string",
        Some(16) => "number",
        Some(17) => "boolean",
        Some(18) => "array",
        Some(19) => "object",
        Some(20) => "key",
        Some(21) => "null",
        Some(22) => "enum member",
        Some(23) => "struct",
        Some(24) => "event",
        Some(25) => "operator",
        Some(26) => "type parameter",
        _ => "symbol",
    }
}

/// The stretch a server writes as `range`.
fn span_of(range: &Value) -> Option<Span> {
    let position = |value: &Value| {
        Some(Position::new(
            u32::try_from(value.get("line")?.as_u64()?).ok()?,
            u32::try_from(value.get("character")?.as_u64()?).ok()?,
        ))
    };
    Some(Span::new(
        position(range.get("start")?)?,
        position(range.get("end")?)?,
    ))
}

/// The symbols in a server's answer to a document symbol request about the file at `file`, outermost first and each followed by those inside it, whether the server answers with nested symbols or a flat list.
pub(crate) fn document_symbols(answer: &Value, file: &Path) -> Vec<Outline> {
    let mut found = Vec::new();
    for item in answer.as_array().into_iter().flatten() {
        if item.get("location").is_some() {
            found.extend(information(item));
        } else {
            nested(item, file, 0, None, &mut found);
        }
    }
    found
}

/// Adds the nested document symbol `item` of the file at `file`, at `depth`, inside the symbol named `container`, and those inside it after it, to `found`.
fn nested(
    item: &Value,
    file: &Path,
    depth: u32,
    container: Option<&str>,
    found: &mut Vec<Outline>,
) {
    let Some(name) = item.get("name").and_then(Value::as_str) else {
        return;
    };
    let span = item
        .get("selectionRange")
        .or_else(|| item.get("range"))
        .and_then(span_of)
        .unwrap_or_default();
    found.push(Outline {
        name: name.to_owned(),
        kind: kind_name(item.get("kind").and_then(Value::as_u64)).to_owned(),
        detail: item
            .get("detail")
            .and_then(Value::as_str)
            .filter(|detail| !detail.is_empty())
            .map(str::to_owned),
        container: container.map(str::to_owned),
        depth,
        file: file.to_path_buf(),
        span,
    });
    for child in item
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        nested(child, file, depth + 1, Some(name), found);
    }
}

/// The symbol a server writes as the flat symbol information `item`, placed in the file its location names; nothing when it names no file.
fn information(item: &Value) -> Option<Outline> {
    let location = item.get("location")?;
    let file = path_of(location.get("uri")?.as_str()?)?;
    Some(Outline {
        name: item.get("name")?.as_str()?.to_owned(),
        kind: kind_name(item.get("kind").and_then(Value::as_u64)).to_owned(),
        detail: None,
        container: item
            .get("containerName")
            .and_then(Value::as_str)
            .filter(|container| !container.is_empty())
            .map(str::to_owned),
        depth: 0,
        file,
        span: location.get("range").and_then(span_of).unwrap_or_default(),
    })
}

/// The symbols in a server's answer to a workspace symbol request.
pub(crate) fn project_symbols(answer: &Value) -> Vec<Outline> {
    answer
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(information)
        .collect()
}

#[cfg(test)]
/// Tests for reading symbols.
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    /// Nested symbols come outermost first with their depth and container, placed by the stretch of their name, and flat ones in the file their location names.
    #[test]
    fn reads_nested_and_flat_symbols() {
        let file = PathBuf::from("/w/src/lib.rs");
        let range = |line: u32| json!({"start": {"line": line, "character": 4}, "end": {"line": line, "character": 9}});
        let answer = json!([{
            "name": "Config", "kind": 23, "detail": "", "range": range(0), "selectionRange": range(0),
            "children": [{"name": "load", "kind": 6, "detail": "fn() -> Self", "range": range(2), "selectionRange": range(2)}]
        }]);
        let found = document_symbols(&answer, &file);
        assert_eq!(found.len(), 2);
        assert_eq!(
            (
                found[0].kind.as_str(),
                found[0].depth,
                found[0].detail.as_deref()
            ),
            ("struct", 0, None)
        );
        assert_eq!((found[1].name.as_str(), found[1].depth), ("load", 1));
        assert_eq!(found[1].container.as_deref(), Some("Config"));
        assert_eq!(found[1].span.start, Position::new(2, 4));
        let flat = json!([{"name": "helper", "kind": 12, "containerName": "util", "location": {"uri": "file:///w/src/util.rs", "range": range(5)}}]);
        let found = project_symbols(&flat);
        assert_eq!(found[0].file, PathBuf::from("/w/src/util.rs"));
        assert_eq!(
            (found[0].kind.as_str(), found[0].container.as_deref()),
            ("function", Some("util"))
        );
        assert_eq!(document_symbols(&flat, &file)[0].name, "helper");
    }
}
