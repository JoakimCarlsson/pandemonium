//! Editable notebook cells and inline MIME outputs in ordinary panes.

use std::path::Path;

use base64::{Engine, engine::general_purpose::STANDARD};
use pm_core::notebook::{CellId, multiline};
use pm_gfx::Image;
use pm_ui::{
    Div, Font, Styled, TextSize, Theme, button, h_flex, paragraph, picture, scroll_area, text,
    v_flex,
};
use serde_json::Value;

use crate::editor::FileId;
use crate::image::Decoding;
use crate::input::bare_input_view;
use crate::markdown::{Renders, blocks, markdown_content};
use crate::message::Message;
use crate::notebook::store::{Action, Entry, Notebooks};

/// Builds a notebook using the existing text, Markdown and GPU image paths.
#[allow(clippy::too_many_arguments)]
pub fn notebook_pane(
    theme: &Theme,
    pane: crate::panes::PaneId,
    file: FileId,
    entry: &Entry,
    notebooks: &Notebooks,
    renders: &Renders,
    path: &Path,
    scale: f32,
    focused: Option<CellId>,
    solid: bool,
) -> Div<Message> {
    let action = |action| Message::Notebook(file, action);
    let mut header = v_flex()
        .w_full()
        .p(1)
        .gap(1)
        .bg(theme.colors.surface)
        .child(
            h_flex()
                .gap(1)
                .items_center()
                .child(button("Run All", action(Action::RunAll)).ghost())
                .child(button("Start", action(Action::Start)).ghost())
                .child(button("Interrupt", action(Action::Interrupt)).ghost())
                .child(button("Restart", action(Action::Restart)).ghost())
                .child(button("Shut Down", action(Action::Shutdown)).ghost())
                .child(
                    text(entry.state.clone())
                        .text_sm()
                        .color(theme.colors.text_muted),
                ),
        )
        .child(
            h_flex()
                .gap(1)
                .child(button("+ Code", action(Action::AddCode(None))).ghost())
                .child(button("+ Markdown", action(Action::AddMarkdown(None))).ghost())
                .child(button("Clear", action(Action::Clear)).ghost())
                .child(button("Save", action(Action::Save)).ghost())
                .child(button("JSON", action(Action::Json)).ghost())
                .child(button("Refresh", action(Action::Refresh)).ghost()),
        );
    let kernels = h_flex()
        .gap(1)
        .items_center()
        .child(text("Kernel:").text_sm())
        .children(entry.specs.iter().enumerate().map(|(index, spec)| {
            let selected = spec["name"].as_str() == entry.selected.as_deref();
            let label = spec["display_name"].as_str().unwrap_or("Unknown kernel");
            button(
                if selected {
                    format!("✓ {label}")
                } else {
                    label.to_owned()
                },
                action(Action::SelectKernel(index)),
            )
            .ghost()
        }));
    header = header.child(kernels);
    if let Some(error) = &entry.error {
        header = header.child(note(theme, error));
    }
    if let Some(name) = &entry.selected
        && !entry.specs.is_empty()
        && !entry.specs.iter().any(|spec| spec["name"] == *name)
    {
        header = header.child(note(
            theme,
            &format!("Saved kernel '{name}' is unavailable. Select an installed kernel above."),
        ));
    }
    let body = match &entry.document {
        Err(error) => v_flex().p(3).child(note(theme, error)),
        Ok(document) => v_flex()
            .w_full()
            .max_w_px(1000.0)
            .mx_auto()
            .p(2)
            .gap(2)
            .children(document.cells.iter().map(|cell| {
                let id = cell.id;
                let count = cell.data["execution_count"]
                    .as_u64()
                    .map_or_else(|| " ".into(), |count| count.to_string());
                let label = if cell.kind() == "code" {
                    format!(
                        "In [{count}]{}",
                        if cell.running {
                            " · running / queued"
                        } else {
                            ""
                        }
                    )
                } else {
                    cell.kind().to_owned()
                };
                let mut card = v_flex()
                    .w_full()
                    .p(1.5)
                    .gap(1)
                    .border_1(theme.colors.border)
                    .rounded(theme.radius.md)
                    .child(
                        h_flex()
                            .gap(1)
                            .items_center()
                            .child(text(label).text_sm().font_mono())
                            .when(cell.kind() == "code", |row| {
                                row.child(button("Run", action(Action::Run(id))).ghost())
                            })
                            .child(button("↑", action(Action::Up(id))).ghost())
                            .child(button("↓", action(Action::Down(id))).ghost())
                            .child(
                                button(
                                    if cell.kind() == "code" {
                                        "Markdown"
                                    } else {
                                        "Code"
                                    },
                                    action(Action::ChangeType(id)),
                                )
                                .ghost(),
                            )
                            .child(button("Delete", action(Action::Delete(id))).ghost())
                            .child(button("+", action(Action::AddCode(Some(id)))).ghost()),
                    );
                if let Some(input) = entry.inputs.get(&id) {
                    card = card.child(
                        bare_input_view(
                            input,
                            focused == Some(id),
                            solid,
                            "Cell source",
                            move |phase, anchor, head| {
                                Message::PointNotebook(pane, file, id, phase, anchor, head)
                            },
                            move |event, step| Message::ScrollNotebookCell(file, id, event, step),
                            Message::ShowInputMenu,
                        )
                        .h_px(theme.size.control * (input.rows() as f32).clamp(2.0, 18.0)),
                    );
                }
                if cell.kind() == "markdown" {
                    card = card.child(markdown_content(
                        theme,
                        &blocks::blocks(&cell.source()),
                        file,
                        scale,
                        path,
                        renders,
                    ));
                }
                card.children(cell.outputs().iter().map(|output| {
                    output_view(theme, file, output, notebooks, renders, path, scale)
                }))
            })),
    };
    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(header)
        .child(
            scroll_area(entry.scroll.clone(), body)
                .selectable()
                .w_full()
                .flex_1(),
        )
}

/// Draws an output using the richest supported representation and a visible fallback.
fn output_view(
    theme: &Theme,
    file: FileId,
    output: &Value,
    notebooks: &Notebooks,
    renders: &Renders,
    path: &Path,
    scale: f32,
) -> Div<Message> {
    match output["output_type"].as_str().unwrap_or("") {
        "stream" => output_text(theme, &multiline(&output["text"])),
        "error" => {
            let trace = output["traceback"]
                .as_array()
                .map(|lines| {
                    lines
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            output_text(
                theme,
                &format!(
                    "{}: {}\n{trace}",
                    output["ename"].as_str().unwrap_or("Error"),
                    output["evalue"].as_str().unwrap_or("")
                ),
            )
        }
        "display_data" | "execute_result" => {
            let data = &output["data"];
            for mime in ["image/png", "image/jpeg", "image/svg+xml"] {
                if data[mime].is_string() || data[mime].is_array() {
                    let source = multiline(&data[mime]);
                    let key = (file, mime.to_owned(), source.clone());
                    let vector = mime == "image/svg+xml";
                    let state = notebooks.pictures.get_image(&key, move || {
                        if vector {
                            Image::from_svg(&source, scale).ok_or("Invalid SVG output".into())
                        } else {
                            STANDARD
                                .decode(
                                    source
                                        .chars()
                                        .filter(|ch| !ch.is_whitespace())
                                        .collect::<String>(),
                                )
                                .map_err(|error| error.to_string())
                                .and_then(|bytes| {
                                    Image::decode(&bytes).ok_or("Invalid image output".into())
                                })
                        }
                    });
                    return match state {
                        Decoding::Ready(image) => v_flex().w_full().child(picture(image)),
                        Decoding::Pending => note(theme, "Decoding image…"),
                        Decoding::Failed(error) => {
                            note(theme, &format!("{error}; output data is preserved"))
                        }
                    };
                }
            }
            if data["text/markdown"].is_string() || data["text/markdown"].is_array() {
                return markdown_content(
                    theme,
                    &blocks::blocks(&multiline(&data["text/markdown"])),
                    file,
                    scale,
                    path,
                    renders,
                );
            }
            if data["text/html"].is_string() || data["text/html"].is_array() {
                return markdown_content(
                    theme,
                    &blocks::blocks(&html_markdown(&multiline(&data["text/html"]))),
                    file,
                    scale,
                    path,
                    renders,
                );
            }
            let unsupported = data
                .as_object()
                .map(|data| {
                    data.keys()
                        .filter(|mime| mime.as_str() != "text/plain")
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut body = v_flex().w_full();
            if !unsupported.is_empty() {
                body = body.child(note(
                    theme,
                    &format!(
                        "Unsupported output: {} · data preserved",
                        unsupported.join(", ")
                    ),
                ));
            }
            if data["text/plain"].is_string() || data["text/plain"].is_array() {
                body = body.child(output_text(theme, &multiline(&data["text/plain"])));
            } else if unsupported.is_empty() {
                body = body.child(note(
                    theme,
                    "Output has no supported representation · data preserved",
                ));
            }
            body
        }
        _ => note(theme, "Unsupported output type · data preserved"),
    }
}

/// Draws wrapped selectable output after removing terminal control sequences.
fn output_text(theme: &Theme, value: &str) -> Div<Message> {
    v_flex().w_full().child(paragraph().w_full().span(
        without_ansi(value),
        Font::new(TextSize::Sm).mono(),
        theme.colors.text,
    ))
}

/// Draws a wrapped pane status or fallback message.
fn note(theme: &Theme, value: &str) -> Div<Message> {
    v_flex().w_full().child(paragraph().w_full().span(
        value.to_owned(),
        Font::new(TextSize::Sm),
        theme.colors.text_muted,
    ))
}

/// Removes ANSI CSI and OSC escapes while keeping printable traceback text.
fn without_ansi(source: &str) -> String {
    let mut chars = source.chars().peekable();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            result.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for ch in chars.by_ref() {
                    if ('@'..='~').contains(&ch) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(ch) = chars.next() {
                    if ch == '\u{7}' || (ch == '\u{1b}' && chars.next() == Some('\\')) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    result
}

/// Converts inert HTML text formatting to the existing Markdown rendering path.
fn html_markdown(source: &str) -> String {
    let mut result = String::new();
    let mut rest = source;
    let mut hidden = None;
    while let Some(open) = rest.find('<') {
        if hidden.is_none() {
            result.push_str(&rest[..open]);
        }
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let tag = rest[open + 1..open + close]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        rest = &rest[open + close + 1..];
        if matches!(tag.as_str(), "script" | "style") {
            hidden = Some(format!("/{tag}"));
        }
        if hidden.as_deref() == Some(tag.as_str()) {
            hidden = None;
            continue;
        }
        if hidden.is_some() {
            continue;
        }
        result.push_str(match tag.as_str() {
            "b" | "/b" | "strong" | "/strong" => "**",
            "i" | "/i" | "em" | "/em" => "*",
            "code" | "/code" => "`",
            "pre" => "\n\n```\n",
            "/pre" => "\n```\n\n",
            "h1" => "\n\n# ",
            "h2" => "\n\n## ",
            "h3" => "\n\n### ",
            "br" | "br/" | "/tr" => "\n",
            "p" | "/p" | "div" | "/div" | "/h1" | "/h2" | "/h3" => "\n\n",
            "li" => "\n- ",
            "td" | "th" => "  ",
            _ => "",
        });
    }
    if hidden.is_none() {
        result.push_str(rest);
    }
    result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}
