//! Markup abbreviations written out as HTML.

mod numbering;
mod parse;
mod tables;

use super::snippet::escape;
use parse::{Element, Node};

/// Stands in the text for a tab stop until every stop is known and can be numbered.
const STOP: char = '\u{1}';

/// Which repetition of a multiplied node is being written, for item numbering.
#[derive(Clone, Copy)]
struct Context {
    /// The position of this repetition, counted from zero.
    index: usize,
    /// How many repetitions there are.
    count: usize,
}

/// One written element or text and the lines it takes.
struct Fragment {
    /// The lines, nested ones already indented.
    lines: Vec<String>,
    /// Whether it must be on lines of its own instead of sitting among its siblings.
    block: bool,
}

/// The markup `abbreviation` expands to, its nested lines indented with `indent`, or nothing when it is not an abbreviation.
pub(super) fn expand(abbreviation: &str, indent: &str) -> Option<String> {
    if abbreviation == "!" {
        return Some(document(indent));
    }
    let nodes = parse::parse(abbreviation)?;
    let writer = Writer { indent };
    let fragments = writer.fragments(&nodes, "", Context { index: 0, count: 1 });
    let lines = if fragments.iter().all(|fragment| !fragment.block) {
        vec![join_inline(&fragments)]
    } else {
        fragments
            .into_iter()
            .flat_map(|fragment| fragment.lines)
            .collect()
    };
    Some(number_stops(&lines.join("\n")))
}

/// The page skeleton `!` stands for.
fn document(indent: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n{indent}<meta charset=\"UTF-8\">\n{indent}<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n{indent}<title>${{1:Document}}</title>\n</head>\n<body>\n{indent}$0\n</body>\n</html>"
    )
}

/// The first line of every fragment run together.
fn join_inline(fragments: &[Fragment]) -> String {
    fragments
        .iter()
        .map(|fragment| fragment.lines[0].as_str())
        .collect()
}

/// `text` with each stop marker turned into `$1`, `$2` and so on, the last one into `$0`; a final `$0` is added when there is no stop.
fn number_stops(text: &str) -> String {
    let total = text.matches(STOP).count();
    if total == 0 {
        return format!("{text}$0");
    }
    let mut numbered = String::with_capacity(text.len() + total * 2);
    let mut seen = 0;
    for character in text.chars() {
        if character == STOP {
            seen += 1;
            let number = if seen == total { 0 } else { seen };
            numbered.push_str(&format!("${number}"));
        } else {
            numbered.push(character);
        }
    }
    numbered
}

/// Writes nodes as fragments, indenting nested lines with one `indent` per depth.
struct Writer<'a> {
    /// What one level of nesting is indented with.
    indent: &'a str,
}

impl Writer<'_> {
    /// Writes `nodes` and every repetition of them, inside an element named `parent`.
    fn fragments(&self, nodes: &[Node], parent: &str, context: Context) -> Vec<Fragment> {
        let mut fragments = Vec::new();
        for node in nodes {
            match node {
                Node::Element(element) => {
                    let times = element.repeat.unwrap_or(1);
                    for index in 0..times {
                        let inner = repetition(element.repeat, index, times, context);
                        fragments.push(self.element(element, parent, inner));
                    }
                }
                Node::Group { items, repeat } => {
                    let times = repeat.unwrap_or(1);
                    for index in 0..times {
                        let inner = repetition(*repeat, index, times, context);
                        fragments.extend(self.fragments(items, parent, inner));
                    }
                }
            }
        }
        fragments
    }

    /// Writes one element with whatever lies inside it.
    fn element(&self, element: &Element, parent: &str, context: Context) -> Fragment {
        let number = |text: &str| numbering::number(text, context.index, context.count);
        let text = element
            .text
            .as_deref()
            .map(|text| escape(&number(text)))
            .filter(|text| !text.is_empty());
        let bare = element.name.is_none()
            && element.id.is_none()
            && element.classes.is_empty()
            && element.attributes.is_empty();
        if bare {
            return Fragment {
                lines: vec![text.unwrap_or_default()],
                block: false,
            };
        }
        let written = element
            .name
            .as_deref()
            .unwrap_or_else(|| tables::implicit_name(parent));
        let (written, input_type) = match written.strip_prefix("input:") {
            Some(kind) => ("input", Some(kind)),
            None => (written, None),
        };
        let tag = tables::resolve(written);
        let open = open_tag(tag, element, input_type, &number);
        if tables::is_void(tag) {
            return Fragment {
                lines: vec![open],
                block: !tables::is_inline(tag),
            };
        }
        let mut children = Vec::new();
        if let Some(text) = text {
            children.push(Fragment {
                lines: vec![text],
                block: false,
            });
        }
        children.extend(self.fragments(&element.children, tag, context));
        let close = format!("</{tag}>");
        let lines = if children.is_empty() {
            vec![format!("{open}{STOP}{close}")]
        } else if children.iter().all(|child| !child.block) {
            vec![format!("{open}{}{close}", join_inline(&children))]
        } else {
            let mut lines = vec![open];
            for child in children {
                for line in child.lines {
                    lines.push(format!("{}{line}", self.indent));
                }
            }
            lines.push(close);
            lines
        };
        Fragment {
            block: !tables::is_inline(tag) || lines.len() > 1,
            lines,
        }
    }
}

/// The numbering of one repetition: its own when the node was multiplied, else the one it is inside.
fn repetition(repeat: Option<usize>, index: usize, count: usize, inherited: Context) -> Context {
    if repeat.is_some() {
        Context { index, count }
    } else {
        inherited
    }
}

/// The opening tag of `tag` with its id, classes, default attributes and written attributes.
fn open_tag(
    tag: &str,
    element: &Element,
    input_type: Option<&str>,
    number: &dyn Fn(&str) -> String,
) -> String {
    let stop = STOP.to_string();
    let mut attributes: Vec<(String, String)> = Vec::new();
    if let Some(id) = &element.id {
        attributes.push(("id".to_string(), escape(&number(id))));
    }
    if !element.classes.is_empty() {
        let classes: Vec<String> = element
            .classes
            .iter()
            .map(|class| escape(&number(class)))
            .collect();
        attributes.push(("class".to_string(), classes.join(" ")));
    }
    let mut written: Vec<(String, Option<String>)> = Vec::new();
    if let Some(kind) = input_type {
        written.push(("type".to_string(), Some(kind.to_string())));
    }
    for attribute in &element.attributes {
        written.push((attribute.name.clone(), attribute.value.clone()));
    }
    for (name, value) in tables::defaults(tag) {
        if written.iter().any(|(given, _)| given == name) {
            continue;
        }
        let value = value.map_or_else(|| stop.clone(), escape);
        attributes.push(((*name).to_string(), value));
    }
    for (name, value) in written {
        let value = value.map_or_else(|| stop.clone(), |value| escape(&number(&value)));
        attributes.push((escape(&number(&name)), value));
    }
    let mut open = format!("<{tag}");
    for (name, value) in attributes {
        open.push_str(&format!(" {name}=\"{value}\""));
    }
    open.push('>');
    open
}
