//! What the markup abbreviations know about tag names.

/// Tags written without a closing tag.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Tags that sit in a line of text instead of standing on a line of their own.
const INLINE: &[&str] = &[
    "a", "abbr", "acronym", "b", "bdo", "big", "br", "button", "cite", "code", "del", "dfn", "em",
    "font", "i", "img", "input", "ins", "kbd", "label", "map", "mark", "object", "q", "s", "samp",
    "select", "small", "span", "strike", "strong", "sub", "sup", "textarea", "time", "tt", "u",
    "var",
];

/// Short names and the tags they stand for.
const ALIASES: &[(&str, &str)] = &[
    ("adr", "address"),
    ("art", "article"),
    ("bq", "blockquote"),
    ("btn", "button"),
    ("dlg", "dialog"),
    ("fig", "figure"),
    ("figc", "figcaption"),
    ("ftr", "footer"),
    ("hdr", "header"),
    ("opt", "option"),
    ("optg", "optgroup"),
    ("sect", "section"),
    ("str", "strong"),
    ("tarea", "textarea"),
];

/// Tags whose children are named after them when no name is written, and the tag that is used.
const IMPLICIT_CHILDREN: &[(&str, &str)] = &[
    ("ul", "li"),
    ("ol", "li"),
    ("menu", "li"),
    ("table", "tr"),
    ("thead", "tr"),
    ("tbody", "tr"),
    ("tfoot", "tr"),
    ("tr", "td"),
    ("colgroup", "col"),
    ("select", "option"),
    ("optgroup", "option"),
    ("datalist", "option"),
];

/// Parents whose children are inline when no name is written.
const INLINE_PARENTS: &[&str] = &[
    "a", "abbr", "b", "button", "cite", "code", "em", "h1", "h2", "h3", "h4", "h5", "h6", "i",
    "label", "p", "q", "small", "span", "strong", "u",
];

/// Attributes a tag is written with, a value of `None` leaving a tab stop to fill it in.
pub(super) type Defaults = &'static [(&'static str, Option<&'static str>)];

/// Whether `tag` is written without a closing tag.
pub(super) fn is_void(tag: &str) -> bool {
    VOID.contains(&tag)
}

/// Whether `tag` stays in the line of its siblings.
pub(super) fn is_inline(tag: &str) -> bool {
    INLINE.contains(&tag)
}

/// The tag a short `name` stands for, or `name` itself.
pub(super) fn resolve(name: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, tag)| tag)
}

/// The tag of an element written without a name inside `parent`.
pub(super) fn implicit_name(parent: &str) -> &'static str {
    if let Some((_, tag)) = IMPLICIT_CHILDREN.iter().find(|(name, _)| *name == parent) {
        return tag;
    }
    if INLINE_PARENTS.contains(&parent) {
        return "span";
    }
    "div"
}

/// The attributes `tag` is written with unless the abbreviation gives them.
pub(super) fn defaults(tag: &str) -> Defaults {
    match tag {
        "a" => &[("href", None)],
        "base" => &[("href", None)],
        "form" => &[("action", None)],
        "html" => &[("lang", Some("en"))],
        "iframe" => &[("src", None)],
        "img" => &[("src", None), ("alt", None)],
        "input" => &[("type", Some("text"))],
        "link" => &[("rel", Some("stylesheet")), ("href", None)],
        "meta" => &[("name", None), ("content", None)],
        "script" => &[("src", None)],
        _ => &[],
    }
}
