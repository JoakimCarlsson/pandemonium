//! Which keys do what: the binding table modal editing reads every key
//! against, written as Zed's `assets/keymaps/vim.json` writes it.
//!
//! A binding is a sequence of keystrokes, an action and a `when` clause
//! over where editing stands: which mode, which operator is waiting for a
//! motion, whether an object or a count is being typed. Later bindings win
//! over earlier ones, so the reader's own bindings go on top of these and
//! a binding to `null` takes a key away.

use crate::action::Action;
use crate::key::Keystroke;
use crate::mode::Mode;
use crate::operator::Operator;

/// Where editing stands when a key arrives, for the `when` clauses to read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Situation {
    /// The mode the buffer is in.
    pub mode: Mode,
    /// The operator waiting for its motion, if one is.
    pub operator: Option<Operator>,
    /// Whether `i` or `a` has been typed and an object is expected.
    pub object: bool,
    /// Whether a count is being typed.
    pub count: bool,
}

/// One thing a `when` clause can say about the situation.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Atom {
    /// Normal mode, nothing waiting.
    Normal,
    /// A visual mode, nothing waiting.
    Visual,
    /// Visual block mode.
    Block,
    /// An operator waiting for a motion.
    Operator,
    /// An object expected after `i` or `a`.
    Object,
    /// Insert mode.
    Insert,
    /// Replace mode.
    Replace,
    /// A count being typed.
    Count,
    /// The waiting operator is this one, by its id.
    Op(String),
}

impl Atom {
    /// The atom `word` names.
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "normal" => Self::Normal,
            "visual" => Self::Visual,
            "block" => Self::Block,
            "operator" => Self::Operator,
            "object" => Self::Object,
            "insert" => Self::Insert,
            "replace" => Self::Replace,
            "count" => Self::Count,
            _ => Self::Op(word.strip_prefix("op=")?.to_owned()),
        })
    }

    /// Whether the atom holds in `situation`.
    fn holds(&self, situation: &Situation) -> bool {
        let waiting = situation.operator.is_some() || situation.object;
        match self {
            Self::Normal => situation.mode == Mode::Normal && !waiting,
            Self::Visual => situation.mode.is_visual() && !situation.object,
            Self::Block => situation.mode == Mode::VisualBlock && !situation.object,
            Self::Operator => situation.operator.is_some() && !situation.object,
            Self::Object => situation.object,
            Self::Insert => situation.mode == Mode::Insert,
            Self::Replace => situation.mode == Mode::Replace,
            Self::Count => situation.count,
            Self::Op(id) => {
                !situation.object
                    && situation
                        .operator
                        .is_some_and(|operator| operator.id() == id)
            }
        }
    }
}

/// The condition a binding carries: clauses joined by `||`, each of atoms
/// joined by `&&`, any of them turned over by `!`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct When(Vec<Vec<(bool, Atom)>>);

impl When {
    /// Reads a clause such as `normal || visual` or `operator && op=d`.
    pub(crate) fn parse(written: &str) -> Option<Self> {
        written
            .split("||")
            .map(|clause| {
                clause
                    .split("&&")
                    .map(|atom| {
                        let atom = atom.trim();
                        let (negated, atom) = match atom.strip_prefix('!') {
                            Some(rest) => (true, rest.trim()),
                            None => (false, atom),
                        };
                        Some((negated, Atom::parse(atom)?))
                    })
                    .collect()
            })
            .collect::<Option<Vec<_>>>()
            .map(Self)
    }

    /// Whether the clause holds in `situation`.
    fn holds(&self, situation: &Situation) -> bool {
        self.0.iter().any(|clause| {
            clause
                .iter()
                .all(|(negated, atom)| atom.holds(situation) != *negated)
        })
    }
}

/// One binding: keys, what they do, and when.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Binding {
    /// The keystrokes, in order.
    keys: Vec<Keystroke>,
    /// What they do.
    action: Action,
    /// When they do it.
    when: When,
}

/// What the keys typed so far come to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lookup {
    /// The action a binding of exactly these keys names, if one applies.
    pub exact: Option<Action>,
    /// Whether a longer binding begins with these keys.
    pub longer: bool,
}

/// A binding the reader wrote that could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadBinding {
    /// The binding as it was written.
    pub written: String,
}

/// Every binding in force.
#[derive(Clone, Debug)]
pub(crate) struct Keymap {
    /// The bindings, earliest first; a later one wins.
    bindings: Vec<Binding>,
}

impl Default for Keymap {
    /// The bindings Zed's vim mode ships, as far as they have an action here.
    fn default() -> Self {
        let mut keymap = Self {
            bindings: Vec::new(),
        };
        for (keys, action, when) in DEFAULT {
            let bound = keymap.bind(keys, action, when);
            debug_assert!(bound.is_ok(), "{keys} => {action} ({when})");
        }
        keymap
    }
}

impl Keymap {
    /// Adds a binding on top of those already in force.
    pub(crate) fn bind(&mut self, keys: &str, action: &str, when: &str) -> Result<(), BadBinding> {
        let bad = || BadBinding {
            written: format!("{keys} => {action} ({when})"),
        };
        let binding = Binding {
            keys: Keystroke::parse_sequence(keys)
                .filter(|keys| !keys.is_empty())
                .ok_or_else(bad)?,
            action: Action::named(action).ok_or_else(bad)?,
            when: When::parse(when).ok_or_else(bad)?,
        };
        self.bindings.push(binding);
        Ok(())
    }

    /// What `keys` come to in `situation`.
    pub(crate) fn lookup(&self, keys: &[Keystroke], situation: &Situation) -> Lookup {
        let applying = self
            .bindings
            .iter()
            .rev()
            .filter(|binding| binding.when.holds(situation));
        let mut lookup = Lookup::default();
        let mut decided = false;
        for binding in applying {
            if binding.keys.len() > keys.len() && binding.keys.starts_with(keys) {
                lookup.longer |= binding.action != Action::Nothing;
            }
            if !decided && binding.keys == keys {
                decided = true;
                lookup.exact =
                    Some(binding.action.clone()).filter(|action| *action != Action::Nothing);
            }
        }
        lookup
    }
}

/// Everywhere a motion applies: normal mode, visual mode, after an operator.
const MOTION: &str = "normal || visual || operator";

/// The bindings, in the order Zed's `vim.json` gives them.
const DEFAULT: &[(&str, &str, &str)] = &[
    ("i", "PushObject", "visual || operator"),
    ("a", "PushObject around", "visual || operator"),
    ("left", "Left", MOTION),
    ("h", "Left", MOTION),
    ("backspace", "WrappingLeft", MOTION),
    ("down", "Down", MOTION),
    ("ctrl-j", "Down", MOTION),
    ("j", "Down", MOTION),
    ("ctrl-m", "NextLineStart", MOTION),
    ("+", "NextLineStart", MOTION),
    ("enter", "NextLineStart", MOTION),
    ("-", "PreviousLineStart", MOTION),
    ("tab", "GoForward", "normal"),
    ("ctrl-i", "GoForward", "normal"),
    ("up", "Up", MOTION),
    ("k", "Up", MOTION),
    ("right", "Right", MOTION),
    ("l", "Right", MOTION),
    ("space", "WrappingRight", MOTION),
    ("end", "EndOfLine", MOTION),
    ("$", "EndOfLine", MOTION),
    ("^", "FirstNonWhitespace", MOTION),
    ("_", "StartOfLineDownward", MOTION),
    ("g _", "EndOfLineDownward", MOTION),
    ("G", "EndOfDocument", MOTION),
    ("{", "StartOfParagraph", MOTION),
    ("}", "EndOfParagraph", MOTION),
    ("(", "SentenceBackward", MOTION),
    (")", "SentenceForward", MOTION),
    ("|", "GoToColumn", MOTION),
    ("w", "NextWordStart", MOTION),
    ("e", "NextWordEnd", MOTION),
    ("b", "PreviousWordStart", MOTION),
    ("g e", "PreviousWordEnd", MOTION),
    ("W", "NextWordStart ignore_punctuation", MOTION),
    ("E", "NextWordEnd ignore_punctuation", MOTION),
    ("B", "PreviousWordStart ignore_punctuation", MOTION),
    ("g E", "PreviousWordEnd ignore_punctuation", MOTION),
    ("/", "Search", MOTION),
    ("g /", "pane::DeploySearch", MOTION),
    ("?", "Search backwards", MOTION),
    ("*", "MoveToNext", MOTION),
    ("#", "MoveToPrevious", MOTION),
    ("n", "MoveToNextMatch", MOTION),
    ("N", "MoveToPreviousMatch", MOTION),
    ("%", "Matching", MOTION),
    ("f", "PushFindForward", MOTION),
    ("t", "PushFindForward before", MOTION),
    ("F", "PushFindBackward", MOTION),
    ("T", "PushFindBackward after", MOTION),
    ("m", "PushMark", "normal || visual"),
    ("'", "PushJump line", MOTION),
    ("`", "PushJump", MOTION),
    (";", "RepeatFind", MOTION),
    (",", "RepeatFindReversed", MOTION),
    ("ctrl-o", "GoBack", "normal"),
    ("ctrl-]", "GoToDefinition", "normal"),
    ("escape", "SwitchToNormalMode", MOTION),
    ("ctrl-[", "SwitchToNormalMode", MOTION),
    ("v", "ToggleVisual", "normal || visual"),
    ("V", "ToggleVisualLine", "normal || visual"),
    ("ctrl-v", "ToggleVisualBlock", "normal || visual"),
    ("ctrl-q", "ToggleVisualBlock", "normal || visual"),
    ("K", "Hover", "normal"),
    ("R", "ToggleReplace", "normal"),
    ("0", "StartOfLine", MOTION),
    ("home", "StartOfLine", MOTION),
    ("ctrl-f", "PageDown", MOTION),
    ("pagedown", "PageDown", MOTION),
    ("ctrl-b", "PageUp", MOTION),
    ("pageup", "PageUp", MOTION),
    ("ctrl-d", "ScrollDown", MOTION),
    ("ctrl-u", "ScrollUp", MOTION),
    ("ctrl-e", "LineDown", "normal || visual"),
    ("ctrl-y", "LineUp", "normal || visual"),
    ("g R", "PushReplaceWithRegister", "normal"),
    ("g r n", "Rename", "normal"),
    ("g r r", "FindAllReferences", "normal"),
    ("g r i", "GoToImplementation", "normal"),
    ("g r a", "ToggleCodeActions", "normal"),
    ("g g", "StartOfDocument", MOTION),
    ("g h", "Hover", "normal"),
    ("g d", "GoToDefinition", "normal"),
    ("g D", "GoToDeclaration", "normal"),
    ("g y", "GoToTypeDefinition", "normal"),
    ("g I", "GoToImplementation", "normal"),
    ("g n", "SelectNextMatch", "normal || visual || operator"),
    ("g N", "SelectPreviousMatch", "normal || visual || operator"),
    ("g l", "SelectNext", "normal || visual"),
    ("g a", "editor::SelectAllMatches", "normal || visual"),
    ("g s", "outline::Toggle", "normal"),
    ("g O", "outline::Toggle", "normal"),
    ("g S", "project_symbols::Toggle", "normal"),
    ("g .", "ToggleCodeActions", "normal"),
    ("g A", "FindAllReferences", "normal"),
    ("g *", "MoveToNext partial_word", MOTION),
    ("g #", "MoveToPrevious partial_word", MOTION),
    ("g j", "Down", MOTION),
    ("g down", "Down", MOTION),
    ("g k", "Up", MOTION),
    ("g up", "Up", MOTION),
    ("g $", "EndOfLine", MOTION),
    ("g end", "EndOfLine", MOTION),
    ("g 0", "StartOfLine", MOTION),
    ("g home", "StartOfLine", MOTION),
    ("g M", "MiddleOfLine", MOTION),
    ("g ^", "FirstNonWhitespace", MOTION),
    ("g v", "RestoreVisualSelection", "normal || visual"),
    ("g ]", "GoToDiagnostic", "normal"),
    ("g [", "GoToPreviousDiagnostic", "normal"),
    ("g i", "InsertAtPrevious", "normal"),
    ("H", "WindowTop", MOTION),
    ("M", "WindowMiddle", MOTION),
    ("L", "WindowBottom", MOTION),
    ("q", "ToggleRecord", "normal"),
    ("Q", "ReplayLastRecording", "normal"),
    ("@", "PushReplayRegister", "normal"),
    ("z enter", "SendKeystrokes z t ^", "normal || visual"),
    ("z -", "SendKeystrokes z b ^", "normal || visual"),
    ("z t", "ScrollCursorTop", "normal || visual"),
    ("z z", "ScrollCursorCenter", "normal || visual"),
    ("z .", "SendKeystrokes z z ^", "normal || visual"),
    ("z b", "ScrollCursorBottom", "normal || visual"),
    ("z a", "ToggleFold", "normal || visual"),
    ("z c", "ToggleFold", "normal || visual"),
    ("z o", "ToggleFold", "normal || visual"),
    ("z M", "FoldAll", "normal || visual"),
    ("z R", "UnfoldAll", "normal || visual"),
    ("z l", "ColumnRight", "normal || visual"),
    ("z h", "ColumnLeft", "normal || visual"),
    ("z L", "HalfPageRight", "normal || visual"),
    ("z H", "HalfPageLeft", "normal || visual"),
    ("Z Q", "CloseActiveItem", "normal"),
    ("Z Z", "SendKeystrokes : x enter", "normal"),
    ("1", "Number 1", MOTION),
    ("2", "Number 2", MOTION),
    ("3", "Number 3", MOTION),
    ("4", "Number 4", MOTION),
    ("5", "Number 5", MOTION),
    ("6", "Number 6", MOTION),
    ("7", "Number 7", MOTION),
    ("8", "Number 8", MOTION),
    ("9", "Number 9", MOTION),
    ("ctrl-w d", "GoToDefinition", "normal"),
    (".", "Repeat", "normal"),
    ("] ]", "NextSectionStart", MOTION),
    ("] [", "NextSectionEnd", MOTION),
    ("[ [", "PreviousSectionStart", MOTION),
    ("[ ]", "PreviousSectionEnd", MOTION),
    ("] m", "NextMethodStart", MOTION),
    ("] M", "NextMethodEnd", MOTION),
    ("[ m", "PreviousMethodStart", MOTION),
    ("[ M", "PreviousMethodEnd", MOTION),
    ("[ *", "PreviousComment", MOTION),
    ("[ /", "PreviousComment", MOTION),
    ("] *", "NextComment", MOTION),
    ("] /", "NextComment", MOTION),
    ("[ -", "PreviousLesserIndent", MOTION),
    ("[ +", "PreviousGreaterIndent", MOTION),
    ("[ =", "PreviousSameIndent", MOTION),
    ("] -", "NextLesserIndent", MOTION),
    ("] +", "NextGreaterIndent", MOTION),
    ("] =", "NextSameIndent", MOTION),
    ("] b", "ActivateNextItem", "normal || visual"),
    ("[ b", "ActivatePreviousItem", "normal || visual"),
    ("] space", "InsertEmptyLineBelow", "normal"),
    ("[ space", "InsertEmptyLineAbove", "normal"),
    ("[ e", "editor::MoveLineUp", "normal || visual"),
    ("] e", "editor::MoveLineDown", "normal || visual"),
    ("] }", "UnmatchedForward }", MOTION),
    ("[ {", "UnmatchedBackward {", MOTION),
    ("] )", "UnmatchedForward )", MOTION),
    ("[ (", "UnmatchedBackward (", MOTION),
    ("i", "InsertBefore", "normal"),
    ("a", "InsertAfter", "normal"),
    (":", "CommandLine", "normal || visual"),
    ("c", "PushChange", "normal"),
    ("C", "ChangeToEndOfLine", "normal"),
    ("d", "PushDelete", "normal"),
    ("delete", "DeleteRight", "normal"),
    ("g J", "JoinLinesNoWhitespace", "normal"),
    ("y", "PushYank", "normal"),
    ("Y", "YankLine", "normal"),
    ("x", "DeleteRight", "normal"),
    ("X", "DeleteLeft", "normal"),
    ("ctrl-a", "Increment", "normal"),
    ("ctrl-x", "Decrement", "normal"),
    ("ctrl-r", "Redo", "normal"),
    (">", "PushIndent", "normal"),
    ("<", "PushOutdent", "normal"),
    ("=", "PushAutoIndent", "normal"),
    ("g u", "PushLowercase", "normal"),
    ("g U", "PushUppercase", "normal"),
    ("g ~", "PushOppositeCase", "normal"),
    ("g ?", "PushRot13", "normal"),
    ("g w", "PushRewrapKeep", "normal"),
    ("g q", "PushRewrap", "normal"),
    ("insert", "InsertBefore", "normal"),
    ("] d", "GoToDiagnostic", "normal"),
    ("[ d", "GoToPreviousDiagnostic", "normal"),
    ("] c", "GoToHunk", "normal"),
    ("[ c", "GoToPreviousHunk", "normal"),
    ("g c", "PushToggleComments", "normal"),
    (
        "0",
        "Number 0",
        "count && normal || count && visual || count && operator",
    ),
    (
        "%",
        "GoToPercentage",
        "count && normal || count && visual || count && operator",
    ),
    ("u", "ConvertToLowerCase", "visual"),
    ("U", "ConvertToUpperCase", "visual"),
    ("O", "OtherEndRowAware", "visual"),
    ("o", "OtherEnd", "visual"),
    ("d", "PushDelete", "visual"),
    ("x", "PushDelete", "visual"),
    ("delete", "PushDelete", "visual"),
    ("D", "VisualDeleteLine", "visual"),
    ("X", "VisualDeleteLine", "visual"),
    ("y", "PushYank", "visual"),
    ("Y", "VisualYankLine", "visual"),
    ("p", "Paste", "visual"),
    ("P", "Paste preserve_clipboard", "visual"),
    ("c", "PushChange", "visual"),
    ("s", "PushChange", "visual"),
    ("R", "VisualChangeLine", "visual"),
    ("S", "PushAddSurrounds", "visual"),
    ("C", "VisualChangeLine", "visual"),
    ("~", "ChangeCase", "visual"),
    ("ctrl-a", "Increment", "visual"),
    ("ctrl-x", "Decrement", "visual"),
    ("g ctrl-a", "Increment step", "visual"),
    ("g ctrl-x", "Decrement step", "visual"),
    ("I", "VisualInsertBefore", "visual"),
    ("A", "VisualInsertAfter", "visual"),
    ("g I", "VisualInsertFirstNonWhiteSpace", "visual"),
    ("g A", "VisualInsertEndOfLine", "visual"),
    ("J", "JoinLines", "visual"),
    ("g J", "JoinLinesNoWhitespace", "visual"),
    ("r", "PushReplace", "visual"),
    (">", "PushIndent", "visual"),
    ("<", "PushOutdent", "visual"),
    ("=", "PushAutoIndent", "visual"),
    ("g R", "Paste preserve_clipboard", "visual"),
    ("g c", "PushToggleComments", "visual"),
    ("g q", "PushRewrap", "visual"),
    ("g w", "PushRewrapKeep", "visual"),
    ("g ?", "ConvertToRot13", "visual"),
    ("g u", "ConvertToLowerCase", "visual"),
    ("g U", "ConvertToUpperCase", "visual"),
    ("g ~", "ChangeCase", "visual"),
    ("\"", "PushRegister", "normal || visual"),
    ("D", "DeleteToEndOfLine", "normal"),
    ("J", "JoinLines", "normal"),
    ("I", "InsertFirstNonWhitespace", "normal"),
    ("A", "InsertEndOfLine", "normal"),
    ("o", "InsertLineBelow", "normal"),
    ("O", "InsertLineAbove", "normal"),
    ("~", "ChangeCase", "normal"),
    ("p", "Paste", "normal"),
    ("P", "Paste before", "normal"),
    ("u", "Undo", "normal"),
    ("r", "PushReplace", "normal"),
    ("s", "Substitute", "normal"),
    ("S", "SubstituteLine", "normal"),
    ("escape", "NormalBefore", "insert"),
    ("ctrl-[", "NormalBefore", "insert"),
    ("ctrl-w", "DeleteToPreviousWordStart", "insert"),
    ("ctrl-u", "DeleteToBeginningOfLine", "insert"),
    ("ctrl-t", "Indent", "insert"),
    ("ctrl-d", "Outdent", "insert"),
    ("ctrl-y", "InsertFromAbove", "insert"),
    ("ctrl-e", "InsertFromBelow", "insert"),
    ("ctrl-r", "PushInsertRegister", "insert"),
    ("insert", "SwitchTyping", "insert || replace"),
    ("ctrl-o", "TemporaryNormal", "insert"),
    ("ctrl-x ctrl-o", "ShowCompletions", "insert"),
    ("ctrl-x ctrl-e", "LineDown", "insert"),
    ("ctrl-x ctrl-y", "LineUp", "insert"),
    ("escape", "NormalBefore", "replace"),
    ("ctrl-[", "NormalBefore", "replace"),
    ("backspace", "UndoReplace", "replace"),
    ("escape", "ClearOperators", "operator || object"),
    ("ctrl-[", "ClearOperators", "operator || object"),
    ("g c", "CurrentLine", "op=gc"),
    ("w", "Word", "object"),
    ("W", "Word ignore_punctuation", "object"),
    ("t", "Tag", "object"),
    ("s", "Sentence", "object"),
    ("p", "Paragraph", "object"),
    ("'", "Quotes", "object"),
    ("`", "BackQuotes", "object"),
    ("\"", "DoubleQuotes", "object"),
    ("q", "MiniQuotes", "object"),
    ("|", "VerticalBars", "object"),
    ("(", "Parentheses", "object"),
    (")", "Parentheses", "object"),
    ("b", "Parentheses", "object"),
    ("[", "SquareBrackets", "object"),
    ("]", "SquareBrackets", "object"),
    ("r", "SquareBrackets", "object"),
    ("{", "CurlyBrackets", "object"),
    ("}", "CurlyBrackets", "object"),
    ("B", "CurlyBrackets", "object"),
    ("<", "AngleBrackets", "object"),
    (">", "AngleBrackets", "object"),
    ("a", "Argument", "object"),
    ("i", "IndentObj", "object"),
    ("I", "IndentObj include_below", "object"),
    ("f", "Method", "object"),
    ("c", "Class", "object"),
    ("e", "EntireFile", "object"),
    ("c", "CurrentLine", "op=c"),
    ("x", "PushExchange", "op=c"),
    ("d", "Rename", "op=c"),
    ("s", "PushChangeSurrounds", "op=c"),
    ("d", "CurrentLine", "op=d"),
    ("s", "PushDeleteSurrounds", "op=d"),
    ("v", "PushForcedMotion", "operator"),
    ("g u", "CurrentLine", "op=gu"),
    ("u", "CurrentLine", "op=gu"),
    ("g U", "CurrentLine", "op=gU"),
    ("U", "CurrentLine", "op=gU"),
    ("g ~", "CurrentLine", "op=g~"),
    ("~", "CurrentLine", "op=g~"),
    ("g ?", "CurrentLine", "op=g?"),
    ("?", "CurrentLine", "op=g?"),
    ("g q", "CurrentLine", "op=gq"),
    ("q", "CurrentLine", "op=gq"),
    ("g w", "CurrentLine", "op=gw"),
    ("w", "CurrentLine", "op=gw"),
    ("y", "CurrentLine", "op=y"),
    ("s", "PushAddSurrounds", "op=y"),
    ("s", "CurrentLine", "op=ys"),
    (">", "CurrentLine", "op=>"),
    ("<", "CurrentLine", "op=<"),
    ("=", "CurrentLine", "op=eq"),
    ("c", "CurrentLine", "op=gc"),
    ("r", "CurrentLine", "op=gR"),
    ("R", "CurrentLine", "op=gR"),
    ("x", "CurrentLine", "op=cx"),
    ("ctrl-w left", "ActivatePaneLeft", "normal || visual"),
    ("ctrl-w right", "ActivatePaneRight", "normal || visual"),
    ("ctrl-w up", "ActivatePaneUp", "normal || visual"),
    ("ctrl-w down", "ActivatePaneDown", "normal || visual"),
    ("ctrl-w ctrl-h", "ActivatePaneLeft", "normal || visual"),
    ("ctrl-w ctrl-l", "ActivatePaneRight", "normal || visual"),
    ("ctrl-w ctrl-k", "ActivatePaneUp", "normal || visual"),
    ("ctrl-w ctrl-j", "ActivatePaneDown", "normal || visual"),
    ("ctrl-w h", "ActivatePaneLeft", "normal || visual"),
    ("ctrl-w l", "ActivatePaneRight", "normal || visual"),
    ("ctrl-w k", "ActivatePaneUp", "normal || visual"),
    ("ctrl-w j", "ActivatePaneDown", "normal || visual"),
    ("ctrl-w g t", "ActivateNextItem", "normal || visual"),
    ("ctrl-w g T", "ActivatePreviousItem", "normal || visual"),
    ("ctrl-w ctrl-v", "SplitVertical", "normal || visual"),
    ("ctrl-w v", "SplitVertical", "normal || visual"),
    ("ctrl-w S", "SplitHorizontal", "normal || visual"),
    ("ctrl-w ctrl-s", "SplitHorizontal", "normal || visual"),
    ("ctrl-w s", "SplitHorizontal", "normal || visual"),
    ("ctrl-w ctrl-c", "CloseActiveItem", "normal || visual"),
    ("ctrl-w c", "CloseActiveItem", "normal || visual"),
    ("ctrl-w ctrl-q", "CloseActiveItem", "normal || visual"),
    ("ctrl-w q", "CloseActiveItem", "normal || visual"),
    ("g t", "GoToTab", "normal || visual"),
    ("g T", "GoToPreviousTab", "normal || visual"),
];
