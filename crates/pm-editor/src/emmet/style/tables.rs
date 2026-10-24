//! What the style abbreviations know about properties, units and keywords.

/// Short names and the properties they stand for.
const PROPERTIES: &[(&str, &str)] = &[
    ("ac", "align-content"),
    ("ai", "align-items"),
    ("anim", "animation"),
    ("ap", "appearance"),
    ("as", "align-self"),
    ("asp", "aspect-ratio"),
    ("b", "bottom"),
    ("bd", "border"),
    ("bdb", "border-bottom"),
    ("bdc", "border-color"),
    ("bdcl", "border-collapse"),
    ("bdl", "border-left"),
    ("bdr", "border-radius"),
    ("bdrs", "border-radius"),
    ("bds", "border-style"),
    ("bdt", "border-top"),
    ("bdw", "border-width"),
    ("bg", "background"),
    ("bgc", "background-color"),
    ("bgi", "background-image"),
    ("bgp", "background-position"),
    ("bgr", "background-repeat"),
    ("bgs", "background-size"),
    ("bxsh", "box-shadow"),
    ("bxz", "box-sizing"),
    ("c", "color"),
    ("cl", "clear"),
    ("con", "content"),
    ("cur", "cursor"),
    ("d", "display"),
    ("fil", "filter"),
    ("fl", "float"),
    ("ff", "font-family"),
    ("fs", "font-style"),
    ("fw", "font-weight"),
    ("fx", "flex"),
    ("fxd", "flex-direction"),
    ("fxg", "flex-grow"),
    ("fxs", "flex-shrink"),
    ("fxw", "flex-wrap"),
    ("fz", "font-size"),
    ("g", "gap"),
    ("gap", "gap"),
    ("gtc", "grid-template-columns"),
    ("gtr", "grid-template-rows"),
    ("h", "height"),
    ("jc", "justify-content"),
    ("l", "left"),
    ("lh", "line-height"),
    ("lis", "list-style"),
    ("ls", "letter-spacing"),
    ("m", "margin"),
    ("mb", "margin-bottom"),
    ("mh", "max-height"),
    ("mih", "min-height"),
    ("miw", "min-width"),
    ("ml", "margin-left"),
    ("mr", "margin-right"),
    ("mt", "margin-top"),
    ("mw", "max-width"),
    ("objf", "object-fit"),
    ("op", "opacity"),
    ("ord", "order"),
    ("out", "outline"),
    ("ov", "overflow"),
    ("ovx", "overflow-x"),
    ("ovy", "overflow-y"),
    ("p", "padding"),
    ("pb", "padding-bottom"),
    ("pe", "pointer-events"),
    ("pl", "padding-left"),
    ("pos", "position"),
    ("pr", "padding-right"),
    ("pt", "padding-top"),
    ("r", "right"),
    ("res", "resize"),
    ("t", "top"),
    ("ta", "text-align"),
    ("td", "text-decoration"),
    ("ti", "text-indent"),
    ("tov", "text-overflow"),
    ("trf", "transform"),
    ("trs", "transition"),
    ("tsh", "text-shadow"),
    ("tt", "text-transform"),
    ("us", "user-select"),
    ("v", "visibility"),
    ("va", "vertical-align"),
    ("w", "width"),
    ("wb", "word-break"),
    ("ws", "white-space"),
    ("z", "z-index"),
];

/// Properties whose bare numbers are written without a unit.
const UNITLESS: &[&str] = &[
    "flex",
    "flex-grow",
    "flex-shrink",
    "font-weight",
    "line-height",
    "opacity",
    "order",
    "z-index",
];

/// Units a number may be followed by, with the long form of the short ones.
const UNITS: &[(&str, &str)] = &[
    ("p", "%"),
    ("e", "em"),
    ("r", "rem"),
    ("x", "ex"),
    ("%", "%"),
    ("px", "px"),
    ("em", "em"),
    ("rem", "rem"),
    ("ex", "ex"),
    ("ch", "ch"),
    ("vw", "vw"),
    ("vh", "vh"),
    ("vmin", "vmin"),
    ("vmax", "vmax"),
    ("pt", "pt"),
    ("pc", "pc"),
    ("cm", "cm"),
    ("mm", "mm"),
    ("in", "in"),
    ("deg", "deg"),
    ("rad", "rad"),
    ("turn", "turn"),
    ("fr", "fr"),
];

/// Units that only mean something for a duration.
const DURATIONS: &[(&str, &str)] = &[("s", "s"), ("ms", "ms")];

/// The keywords one group of properties know by a short name.
struct Keywords {
    /// The properties the short names are for.
    properties: &'static [&'static str],
    /// Short names and the keywords they stand for.
    words: &'static [(&'static str, &'static str)],
}

/// Border styles, shared by every border-like property.
const LINE_STYLES: &[(&str, &str)] = &[
    ("s", "solid"),
    ("d", "dashed"),
    ("dt", "dotted"),
    ("n", "none"),
];

/// Alignment keywords of the flex properties.
const ALIGNMENTS: &[(&str, &str)] = &[
    ("c", "center"),
    ("s", "flex-start"),
    ("e", "flex-end"),
    ("st", "stretch"),
    ("b", "baseline"),
];

/// The short keywords of each property.
const KEYWORDS: &[Keywords] = &[
    Keywords {
        properties: &["display"],
        words: &[
            ("f", "flex"),
            ("if", "inline-flex"),
            ("g", "grid"),
            ("ig", "inline-grid"),
            ("b", "block"),
            ("i", "inline"),
            ("ib", "inline-block"),
            ("n", "none"),
            ("t", "table"),
            ("tc", "table-cell"),
        ],
    },
    Keywords {
        properties: &["position"],
        words: &[
            ("a", "absolute"),
            ("r", "relative"),
            ("f", "fixed"),
            ("s", "sticky"),
            ("st", "static"),
        ],
    },
    Keywords {
        properties: &["text-align"],
        words: &[
            ("l", "left"),
            ("c", "center"),
            ("r", "right"),
            ("j", "justify"),
        ],
    },
    Keywords {
        properties: &["font-weight"],
        words: &[
            ("b", "bold"),
            ("n", "normal"),
            ("l", "lighter"),
            ("br", "bolder"),
        ],
    },
    Keywords {
        properties: &["font-style"],
        words: &[("i", "italic"), ("n", "normal"), ("o", "oblique")],
    },
    Keywords {
        properties: &["text-decoration"],
        words: &[
            ("n", "none"),
            ("u", "underline"),
            ("o", "overline"),
            ("l", "line-through"),
        ],
    },
    Keywords {
        properties: &["text-transform"],
        words: &[
            ("u", "uppercase"),
            ("l", "lowercase"),
            ("c", "capitalize"),
            ("n", "none"),
        ],
    },
    Keywords {
        properties: &["float"],
        words: &[("l", "left"), ("r", "right"), ("n", "none")],
    },
    Keywords {
        properties: &["clear"],
        words: &[("b", "both"), ("l", "left"), ("r", "right"), ("n", "none")],
    },
    Keywords {
        properties: &["overflow", "overflow-x", "overflow-y"],
        words: &[
            ("h", "hidden"),
            ("s", "scroll"),
            ("a", "auto"),
            ("v", "visible"),
        ],
    },
    Keywords {
        properties: &["cursor"],
        words: &[
            ("p", "pointer"),
            ("d", "default"),
            ("t", "text"),
            ("m", "move"),
            ("g", "grab"),
            ("na", "not-allowed"),
        ],
    },
    Keywords {
        properties: &["visibility"],
        words: &[("v", "visible"), ("h", "hidden")],
    },
    Keywords {
        properties: &["white-space"],
        words: &[
            ("n", "normal"),
            ("nw", "nowrap"),
            ("p", "pre"),
            ("pw", "pre-wrap"),
            ("pl", "pre-line"),
        ],
    },
    Keywords {
        properties: &["vertical-align"],
        words: &[
            ("t", "top"),
            ("m", "middle"),
            ("b", "bottom"),
            ("bl", "baseline"),
        ],
    },
    Keywords {
        properties: &[
            "border",
            "border-top",
            "border-bottom",
            "border-left",
            "border-right",
            "border-style",
            "outline",
        ],
        words: LINE_STYLES,
    },
    Keywords {
        properties: &["flex-direction"],
        words: &[
            ("r", "row"),
            ("rr", "row-reverse"),
            ("c", "column"),
            ("cr", "column-reverse"),
        ],
    },
    Keywords {
        properties: &["flex-wrap"],
        words: &[("w", "wrap"), ("n", "nowrap"), ("wr", "wrap-reverse")],
    },
    Keywords {
        properties: &["justify-content"],
        words: &[
            ("c", "center"),
            ("s", "flex-start"),
            ("e", "flex-end"),
            ("sb", "space-between"),
            ("sa", "space-around"),
            ("se", "space-evenly"),
        ],
    },
    Keywords {
        properties: &["align-items", "align-content", "align-self"],
        words: ALIGNMENTS,
    },
    Keywords {
        properties: &["box-sizing"],
        words: &[("b", "border-box"), ("c", "content-box")],
    },
    Keywords {
        properties: &["pointer-events"],
        words: &[("n", "none"), ("a", "auto")],
    },
    Keywords {
        properties: &["user-select"],
        words: &[("n", "none"), ("a", "auto"), ("t", "text")],
    },
    Keywords {
        properties: &["text-overflow"],
        words: &[("e", "ellipsis"), ("c", "clip")],
    },
    Keywords {
        properties: &["background-repeat"],
        words: &[
            ("n", "no-repeat"),
            ("x", "repeat-x"),
            ("y", "repeat-y"),
            ("r", "repeat"),
        ],
    },
    Keywords {
        properties: &["background-size", "object-fit"],
        words: &[("c", "cover"), ("ct", "contain"), ("f", "fill")],
    },
    Keywords {
        properties: &["word-break"],
        words: &[("ba", "break-all"), ("k", "keep-all")],
    },
    Keywords {
        properties: &["resize"],
        words: &[
            ("n", "none"),
            ("b", "both"),
            ("h", "horizontal"),
            ("v", "vertical"),
        ],
    },
    Keywords {
        properties: &["flex", "list-style"],
        words: &[("n", "none"), ("a", "auto")],
    },
];

/// The property a short name stands for.
pub(super) fn alias(name: &str) -> Option<&'static str> {
    PROPERTIES
        .iter()
        .find(|(short, _)| *short == name)
        .map(|(_, property)| *property)
}

/// Whether `name` is a property that a short name stands for, written in full.
pub(super) fn is_property(name: &str) -> bool {
    PROPERTIES.iter().any(|(_, property)| *property == name)
}

/// The property `name` is, as a short name or in full.
pub(super) fn property(name: &str) -> Option<&'static str> {
    alias(name).or_else(|| {
        PROPERTIES
            .iter()
            .map(|(_, property)| *property)
            .find(|property| *property == name)
    })
}

/// The keyword a short `word` stands for in `property`, with `a` meaning `auto` wherever nothing else claims it.
pub(super) fn keyword_alias(property: &str, word: &str) -> Option<&'static str> {
    let specific = KEYWORDS
        .iter()
        .find(|keywords| keywords.properties.contains(&property))
        .and_then(|keywords| keywords.words.iter().find(|(short, _)| *short == word))
        .map(|(_, keyword)| *keyword);
    specific.or((word == "a").then_some("auto"))
}

/// The keyword `word` stands for in `property`, or `word` itself.
pub(super) fn keyword<'a>(property: &str, word: &'a str) -> &'a str {
    keyword_alias(property, word).unwrap_or(word)
}

/// The unit `letters` write after a number in `property`, nothing when they are not a unit there.
pub(super) fn unit(property: &str, letters: &str) -> Option<&'static str> {
    let timed = property.starts_with("transition") || property.starts_with("animation");
    UNITS
        .iter()
        .chain(if timed { DURATIONS } else { &[] })
        .find(|(short, _)| *short == letters)
        .map(|(_, unit)| *unit)
}

/// Whether a bare number in `property` is written without a unit.
pub(super) fn is_unitless(property: &str) -> bool {
    UNITLESS.contains(&property)
}
