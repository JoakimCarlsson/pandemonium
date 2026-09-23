//! What a binding can do: every vim action, named the way Zed's vim keymap
//! names it, so that a binding reads the same in either editor.
//!
//! A name may carry arguments after a space — `NextWordStart
//! ignore_punctuation`, `Number 3`, `SendKeystrokes z t ^` — which is how
//! the table writes Zed's `["vim::NextWordStart", {"ignore_punctuation":
//! true}]` without a second syntax.

use crate::key::Keystroke;
use crate::mode::Mode;
use crate::motion::{Find, Indentation, Motion};
use crate::object::Object;
use crate::operator::Operator;

/// Where `z` puts the cursor's line in the view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    /// `zt`: at the top.
    Top,
    /// `zz`: in the middle.
    Center,
    /// `zb`: at the bottom.
    Bottom,
}

/// A key that waits for the character after it: `f`, `r`, `m`, `"`, `q`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Waiting {
    /// `f`, `t`, `F` or `T`: the character to find.
    Find { forward: bool, till: bool },
    /// `r`: the character to put in.
    Replace,
    /// `m`: the mark to set.
    Mark,
    /// `'` or `` ` ``: the mark to go to.
    Jump { line: bool },
    /// `"`: the register the next command uses.
    Register,
    /// `q`: the register to record into.
    Record,
    /// `@`: the register to play.
    Replay,
    /// Ctrl-R in insert mode: the register to put in.
    InsertRegister,
    /// `ds`: the delimiters to take away.
    DeleteSurround,
    /// `cs`: the delimiters to change.
    ChangeSurround,
    /// `cs` and a pair: the delimiters to put in their place.
    ChangeSurroundTo(char),
    /// `ys` and a target, or `S` in visual mode: the delimiters to add.
    AddSurround,
}

/// Something the window does rather than the text: a pane, a tab, a jump,
/// a question for the language server.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Command {
    /// Write the file.
    Save,
    /// Write every file.
    SaveAll,
    /// Close the file's tab.
    Close,
    /// Split the pane side by side.
    SplitRight,
    /// Split the pane one above the other.
    SplitDown,
    /// Focus the pane to the left.
    FocusLeft,
    /// Focus the pane to the right.
    FocusRight,
    /// Focus the pane above.
    FocusUp,
    /// Focus the pane below.
    FocusDown,
    /// Show the next tab.
    NextTab,
    /// Show the previous tab.
    PreviousTab,
    /// Open again the tab closed last.
    ReopenTab,
    /// Go back along the jump list.
    GoBack,
    /// Go forward along the jump list.
    GoForward,
    /// Go to where the symbol is defined.
    GoToDefinition,
    /// Go to where its type is defined.
    GoToTypeDefinition,
    /// Go to what implements it.
    GoToImplementation,
    /// Go to where it is declared.
    GoToDeclaration,
    /// List where it is used.
    FindReferences,
    /// Show what the server says about it.
    Hover,
    /// Rename it everywhere.
    Rename,
    /// Offer the fixes the server has here.
    CodeActions,
    /// Offer completions here.
    ShowCompletions,
    /// Show the signature of the call the cursor is in.
    ShowSignature,
    /// Go to the next problem.
    NextDiagnostic,
    /// Go to the previous problem.
    PreviousDiagnostic,
    /// Go to the next change against the index.
    NextHunk,
    /// Go to the previous change against the index.
    PreviousHunk,
    /// Fold or unfold what the cursor's line holds.
    ToggleFold,
    /// Fold everything.
    FoldAll,
    /// Unfold everything.
    UnfoldAll,
    /// List the symbols of the file.
    ShowSymbols,
    /// List the symbols of the workspace.
    ShowWorkspaceSymbols,
    /// Open the file finder.
    ShowFiles,
    /// Search every file.
    SearchProject,
    /// Move the cursor's lines up.
    MoveLineUp,
    /// Move the cursor's lines down.
    MoveLineDown,
    /// Select every place the selection appears.
    SelectAllMatches,
    /// Add the next place it appears to the selection.
    SelectNext,
    /// Dismiss whatever is open over the text.
    Cancel,
}

/// A command that is neither a motion, an object nor an operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Act {
    /// Escape: back to normal mode, or out of what is half typed.
    NormalMode,
    /// `i`: type before the cursor.
    InsertBefore,
    /// `a`: type after it.
    InsertAfter,
    /// `I`: type before the line's first non-blank.
    InsertFirstNonBlank,
    /// `A`: type at the line's end.
    InsertEndOfLine,
    /// `o`: type on a new line below.
    InsertLineBelow,
    /// `O`: type on a new line above.
    InsertLineAbove,
    /// `gi`: type where typing last stopped.
    InsertAtPrevious,
    /// `] space`: put an empty line below.
    InsertEmptyLineBelow,
    /// `[ space`: put an empty line above.
    InsertEmptyLineAbove,
    /// `v`, `V` or Ctrl-V: select, or switch or leave the kind of selection.
    ToggleVisual(Mode),
    /// `gv`: select what was selected last.
    RestoreVisual,
    /// `o` in visual mode: go to the selection's other end.
    OtherEnd,
    /// `O` in visual mode: go to the other end of the line, in a block.
    OtherEndRowAware,
    /// `R`: type over the text.
    ToggleReplace,
    /// `x`, `X`, `s`, `S`, `C`, `D`, `Y`: an operator and its target at once.
    Shorthand(Shorthand),
    /// `D`, `X`, `Y`, `S` and the rest in visual mode: an operator on the
    /// selection's whole lines.
    VisualLines(Operator),
    /// `J` or `gJ`: join lines, with a space between for `J`.
    JoinLines { spaces: bool },
    /// `p` or `P`: put a register's text after the cursor, or before; in
    /// visual mode, in place of the selection, keeping the register when
    /// `preserve`.
    Paste { before: bool, preserve: bool },
    /// `u`: take the last change back.
    Undo,
    /// Ctrl-R: put the last change taken back again.
    Redo,
    /// `.`: make the last change again.
    Repeat,
    /// `~`: swap the case of the character under the cursor.
    ChangeCase,
    /// `u` or `U` in visual mode: lower or raise the case of the selection.
    ConvertCase(Operator),
    /// Ctrl-A or Ctrl-X: count the number at the cursor up or down, by a
    /// growing step on each line for `g Ctrl-A`.
    Increment { delta: i64, step: bool },
    /// `I` or `A` in visual mode: type at the start or the end of every line
    /// of the selection, or of its non-blank part.
    VisualInsert { end: bool, text: bool },
    /// `q`: record into a register, or stop recording.
    ToggleRecord,
    /// `Q`: play the register recorded last.
    ReplayLastRecording,
    /// `zt`, `zz` or `zb`: scroll the cursor's line to a place in the view.
    Scroll(Placement),
    /// Ctrl-E or Ctrl-Y: scroll the view by lines, the cursor staying on it.
    ScrollLines(isize),
    /// `zl` or `zh`: scroll the view sideways.
    ScrollColumns(isize),
    /// `gn` or `gN`: select the next match of the last search.
    SelectMatch { forward: bool },
    /// `gt` or `gT`: show the next tab, or the previous one.
    GoToTab { forward: bool },
    /// Ctrl-O in insert mode: one command in normal mode, then back.
    TemporaryNormal,
    /// Ctrl-W in insert mode: delete the word before the cursor.
    DeleteWordBefore,
    /// Ctrl-U in insert mode: delete back to where the line starts.
    DeleteToLineStart,
    /// Ctrl-T or Ctrl-D in insert mode: indent the line, or outdent it.
    ShiftLine { indent: bool },
    /// Ctrl-Y or Ctrl-E in insert mode: type the character above the
    /// cursor, or below it.
    CopyFromLine { above: bool },
    /// Backspace in replace mode: put back what was typed over.
    UndoReplace,
    /// Insert: switch between typing and typing over.
    SwitchTyping,
}

/// The one-key commands that are an operator and a target in one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Shorthand {
    /// `x`: `dl`.
    DeleteRight,
    /// `X`: `dh`.
    DeleteLeft,
    /// `s`: `cl`.
    Substitute,
    /// `S`: `cc`.
    SubstituteLine,
    /// `C`: `c$`.
    ChangeToEndOfLine,
    /// `D`: `d$`.
    DeleteToEndOfLine,
    /// `Y`: `yy`.
    YankLine,
}

/// What a binding does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    /// Nothing: a binding that takes a key away from the ones below it.
    Nothing,
    /// A digit of a count.
    Number(usize),
    /// Begins an operator, which the next motion or object finishes.
    Push(Operator),
    /// `i` or `a` after an operator or in visual mode: an object follows.
    PushObject { around: bool },
    /// Waits for a character.
    Wait(Waiting),
    /// The operator doubled: it acts on whole lines from the cursor's.
    CurrentLine,
    /// `v` after an operator: its motion's kind is turned over.
    ForcedMotion,
    /// Moves the cursor, or finishes the operator waiting.
    Motion(Motion),
    /// Finishes `i` or `a`.
    Object(Object),
    /// `/` or `?`: begins typing a search.
    Search { backwards: bool },
    /// `:`: begins typing a command line.
    CommandLine,
    /// Everything else.
    Act(Act),
    /// Something for the window to do.
    App(Command),
    /// Plays keys as though they had been typed.
    SendKeystrokes(Vec<Keystroke>),
}

impl Action {
    /// The action `written` names, a name and its arguments apart by spaces.
    pub(crate) fn named(written: &str) -> Option<Self> {
        let written = written.trim();
        let written = written.strip_prefix("vim::").unwrap_or(written);
        let (name, args) = written.split_once(' ').unwrap_or((written, ""));
        let has = |flag: &str| args.split_whitespace().any(|arg| arg == flag);
        let big = has("ignore_punctuation");
        let motion = |motion: Motion| Some(Self::Motion(motion));
        let act = |act: Act| Some(Self::Act(act));
        let app = |command: Command| Some(Self::App(command));
        let object = |object: Object| Some(Self::Object(object));
        let shorthand = |shorthand: Shorthand| Some(Self::Act(Act::Shorthand(shorthand)));
        if let Some(operator) = Operator::ALL
            .iter()
            .find(|operator| operator.name() == name)
        {
            return Some(Self::Push(*operator));
        }
        match name {
            "null" | "Nothing" => Some(Self::Nothing),
            "Number" => Some(Self::Number(args.trim().parse().ok()?)),
            "PushObject" => Some(Self::PushObject {
                around: has("around"),
            }),
            "PushFindForward" => Some(Self::Wait(Waiting::Find {
                forward: true,
                till: has("before"),
            })),
            "PushFindBackward" => Some(Self::Wait(Waiting::Find {
                forward: false,
                till: has("after"),
            })),
            "PushReplace" => Some(Self::Wait(Waiting::Replace)),
            "PushMark" => Some(Self::Wait(Waiting::Mark)),
            "PushJump" => Some(Self::Wait(Waiting::Jump { line: has("line") })),
            "PushRegister" => Some(Self::Wait(Waiting::Register)),
            "PushReplayRegister" => Some(Self::Wait(Waiting::Replay)),
            "PushInsertRegister" => Some(Self::Wait(Waiting::InsertRegister)),
            "PushDeleteSurrounds" => Some(Self::Wait(Waiting::DeleteSurround)),
            "PushChangeSurrounds" => Some(Self::Wait(Waiting::ChangeSurround)),
            "PushSurroundSelection" => Some(Self::Wait(Waiting::AddSurround)),
            "CurrentLine" => Some(Self::CurrentLine),
            "PushForcedMotion" => Some(Self::ForcedMotion),
            "Search" => Some(Self::Search {
                backwards: has("backwards"),
            }),
            "CommandLine" | "command_palette::Toggle" => Some(Self::CommandLine),
            "SendKeystrokes" => Some(Self::SendKeystrokes(Keystroke::parse_sequence(args)?)),
            "Left" => motion(Motion::Left),
            "Right" => motion(Motion::Right),
            "WrappingLeft" => motion(Motion::WrappingLeft),
            "WrappingRight" => motion(Motion::WrappingRight),
            "Up" => motion(Motion::Up),
            "Down" => motion(Motion::Down),
            "NextWordStart" => motion(Motion::NextWordStart { big }),
            "NextWordEnd" => motion(Motion::NextWordEnd { big }),
            "PreviousWordStart" => motion(Motion::PreviousWordStart { big }),
            "PreviousWordEnd" => motion(Motion::PreviousWordEnd { big }),
            "StartOfLine" => motion(Motion::LineStart),
            "FirstNonWhitespace" => motion(Motion::FirstNonBlank),
            "EndOfLine" => motion(Motion::LineEnd),
            "EndOfLineDownward" => motion(Motion::LastNonBlank),
            "MiddleOfLine" => motion(Motion::MiddleOfLine),
            "NextLineStart" => motion(Motion::NextLineStart),
            "PreviousLineStart" => motion(Motion::PreviousLineStart),
            "StartOfLineDownward" => motion(Motion::CurrentLineStart),
            "GoToColumn" => motion(Motion::Column),
            "StartOfDocument" => motion(Motion::FirstLine),
            "EndOfDocument" => motion(Motion::LastLine),
            "GoToPercentage" => motion(Motion::Percent),
            "RepeatFind" => motion(Motion::RepeatFind { reverse: false }),
            "RepeatFindReversed" => motion(Motion::RepeatFind { reverse: true }),
            "Matching" => motion(Motion::Matching),
            "StartOfParagraph" => motion(Motion::ParagraphBackward),
            "EndOfParagraph" => motion(Motion::ParagraphForward),
            "SentenceForward" => motion(Motion::SentenceForward),
            "SentenceBackward" => motion(Motion::SentenceBackward),
            "NextSectionStart" => motion(Motion::Section {
                forward: true,
                end: false,
            }),
            "NextSectionEnd" => motion(Motion::Section {
                forward: true,
                end: true,
            }),
            "PreviousSectionStart" => motion(Motion::Section {
                forward: false,
                end: false,
            }),
            "PreviousSectionEnd" => motion(Motion::Section {
                forward: false,
                end: true,
            }),
            "NextMethodStart" => motion(Motion::Method {
                forward: true,
                end: false,
            }),
            "NextMethodEnd" => motion(Motion::Method {
                forward: true,
                end: true,
            }),
            "PreviousMethodStart" => motion(Motion::Method {
                forward: false,
                end: false,
            }),
            "PreviousMethodEnd" => motion(Motion::Method {
                forward: false,
                end: true,
            }),
            "NextComment" => motion(Motion::Comment { forward: true }),
            "PreviousComment" => motion(Motion::Comment { forward: false }),
            "NextLesserIndent" => motion(Motion::Indent {
                forward: true,
                indentation: Indentation::Lesser,
            }),
            "NextGreaterIndent" => motion(Motion::Indent {
                forward: true,
                indentation: Indentation::Greater,
            }),
            "NextSameIndent" => motion(Motion::Indent {
                forward: true,
                indentation: Indentation::Same,
            }),
            "PreviousLesserIndent" => motion(Motion::Indent {
                forward: false,
                indentation: Indentation::Lesser,
            }),
            "PreviousGreaterIndent" => motion(Motion::Indent {
                forward: false,
                indentation: Indentation::Greater,
            }),
            "PreviousSameIndent" => motion(Motion::Indent {
                forward: false,
                indentation: Indentation::Same,
            }),
            "UnmatchedForward" => motion(Motion::Unmatched {
                forward: true,
                bracket: args.trim().chars().next()?,
            }),
            "UnmatchedBackward" => motion(Motion::Unmatched {
                forward: false,
                bracket: args.trim().chars().next()?,
            }),
            "WindowTop" => motion(Motion::ViewTop),
            "WindowMiddle" => motion(Motion::ViewMiddle),
            "WindowBottom" => motion(Motion::ViewBottom),
            "ScrollDown" => motion(Motion::HalfPageDown),
            "ScrollUp" => motion(Motion::HalfPageUp),
            "PageDown" => motion(Motion::PageDown),
            "PageUp" => motion(Motion::PageUp),
            "MoveToNextMatch" => motion(Motion::SearchNext { reverse: false }),
            "MoveToPreviousMatch" => motion(Motion::SearchNext { reverse: true }),
            "MoveToNext" => motion(Motion::SearchWord {
                forward: true,
                partial: has("partial_word"),
            }),
            "MoveToPrevious" => motion(Motion::SearchWord {
                forward: false,
                partial: has("partial_word"),
            }),
            "Word" => object(Object::Word { big }),
            "Sentence" => object(Object::Sentence),
            "Paragraph" => object(Object::Paragraph),
            "Quotes" => object(Object::Quotes('\'')),
            "BackQuotes" => object(Object::Quotes('`')),
            "DoubleQuotes" => object(Object::Quotes('"')),
            "VerticalBars" => object(Object::Quotes('|')),
            "MiniQuotes" | "AnyQuotes" => object(Object::AnyQuotes),
            "Parentheses" => object(Object::Brackets('(', ')')),
            "SquareBrackets" => object(Object::Brackets('[', ']')),
            "CurlyBrackets" => object(Object::Brackets('{', '}')),
            "AngleBrackets" => object(Object::Brackets('<', '>')),
            "Argument" => object(Object::Argument),
            "Tag" => object(Object::Tag),
            "IndentObj" => object(Object::Indent {
                below: has("include_below"),
            }),
            "Method" => object(Object::Function),
            "Class" => object(Object::Class),
            "EntireFile" => object(Object::EntireFile),
            "SwitchToNormalMode" | "NormalBefore" | "ClearOperators" => act(Act::NormalMode),
            "InsertBefore" => act(Act::InsertBefore),
            "InsertAfter" => act(Act::InsertAfter),
            "InsertFirstNonWhitespace" => act(Act::InsertFirstNonBlank),
            "InsertEndOfLine" => act(Act::InsertEndOfLine),
            "InsertLineBelow" => act(Act::InsertLineBelow),
            "InsertLineAbove" => act(Act::InsertLineAbove),
            "InsertAtPrevious" => act(Act::InsertAtPrevious),
            "InsertEmptyLineBelow" => act(Act::InsertEmptyLineBelow),
            "InsertEmptyLineAbove" => act(Act::InsertEmptyLineAbove),
            "ToggleVisual" => act(Act::ToggleVisual(Mode::Visual)),
            "ToggleVisualLine" => act(Act::ToggleVisual(Mode::VisualLine)),
            "ToggleVisualBlock" => act(Act::ToggleVisual(Mode::VisualBlock)),
            "RestoreVisualSelection" => act(Act::RestoreVisual),
            "OtherEnd" => act(Act::OtherEnd),
            "OtherEndRowAware" => act(Act::OtherEndRowAware),
            "ToggleReplace" => act(Act::ToggleReplace),
            "DeleteRight" => shorthand(Shorthand::DeleteRight),
            "DeleteLeft" => shorthand(Shorthand::DeleteLeft),
            "Substitute" => shorthand(Shorthand::Substitute),
            "SubstituteLine" => shorthand(Shorthand::SubstituteLine),
            "ChangeToEndOfLine" => shorthand(Shorthand::ChangeToEndOfLine),
            "DeleteToEndOfLine" => shorthand(Shorthand::DeleteToEndOfLine),
            "YankLine" => shorthand(Shorthand::YankLine),
            "VisualDeleteLine" => act(Act::VisualLines(Operator::Delete)),
            "VisualYankLine" => act(Act::VisualLines(Operator::Yank)),
            "VisualChangeLine" => act(Act::VisualLines(Operator::Change)),
            "JoinLines" => act(Act::JoinLines { spaces: true }),
            "JoinLinesNoWhitespace" => act(Act::JoinLines { spaces: false }),
            "Paste" => act(Act::Paste {
                before: has("before"),
                preserve: has("preserve_clipboard"),
            }),
            "Undo" => act(Act::Undo),
            "Redo" => act(Act::Redo),
            "Repeat" => act(Act::Repeat),
            "ChangeCase" => act(Act::ChangeCase),
            "ConvertToLowerCase" => act(Act::ConvertCase(Operator::Lowercase)),
            "ConvertToUpperCase" => act(Act::ConvertCase(Operator::Uppercase)),
            "ConvertToRot13" => act(Act::ConvertCase(Operator::Rot13)),
            "Increment" => act(Act::Increment {
                delta: 1,
                step: has("step"),
            }),
            "Decrement" => act(Act::Increment {
                delta: -1,
                step: has("step"),
            }),
            "VisualInsertBefore" => act(Act::VisualInsert {
                end: false,
                text: false,
            }),
            "VisualInsertAfter" => act(Act::VisualInsert {
                end: true,
                text: false,
            }),
            "VisualInsertFirstNonWhiteSpace" => act(Act::VisualInsert {
                end: false,
                text: true,
            }),
            "VisualInsertEndOfLine" => act(Act::VisualInsert {
                end: true,
                text: true,
            }),
            "ToggleRecord" => act(Act::ToggleRecord),
            "ReplayLastRecording" => act(Act::ReplayLastRecording),
            "ScrollCursorTop" | "editor::ScrollCursorTop" => act(Act::Scroll(Placement::Top)),
            "ScrollCursorCenter" | "editor::ScrollCursorCenter" => {
                act(Act::Scroll(Placement::Center))
            }
            "ScrollCursorBottom" | "editor::ScrollCursorBottom" => {
                act(Act::Scroll(Placement::Bottom))
            }
            "LineDown" => act(Act::ScrollLines(1)),
            "LineUp" => act(Act::ScrollLines(-1)),
            "ColumnRight" => act(Act::ScrollColumns(1)),
            "ColumnLeft" => act(Act::ScrollColumns(-1)),
            "HalfPageRight" => act(Act::ScrollColumns(20)),
            "HalfPageLeft" => act(Act::ScrollColumns(-20)),
            "SelectNextMatch" => act(Act::SelectMatch { forward: true }),
            "SelectPreviousMatch" => act(Act::SelectMatch { forward: false }),
            "GoToTab" => act(Act::GoToTab { forward: true }),
            "GoToPreviousTab" => act(Act::GoToTab { forward: false }),
            "TemporaryNormal" => act(Act::TemporaryNormal),
            "DeleteToPreviousWordStart" | "editor::DeleteToPreviousWordStart" => {
                act(Act::DeleteWordBefore)
            }
            "DeleteToBeginningOfLine" | "editor::DeleteToBeginningOfLine" => {
                act(Act::DeleteToLineStart)
            }
            "Indent" => act(Act::ShiftLine { indent: true }),
            "Outdent" => act(Act::ShiftLine { indent: false }),
            "InsertFromAbove" => act(Act::CopyFromLine { above: true }),
            "InsertFromBelow" => act(Act::CopyFromLine { above: false }),
            "UndoReplace" => act(Act::UndoReplace),
            "SwitchTyping" => act(Act::SwitchTyping),
            "Save" | "workspace::Save" => app(Command::Save),
            "SaveAll" | "workspace::SaveAll" => app(Command::SaveAll),
            "CloseActiveItem" | "pane::CloseActiveItem" => app(Command::Close),
            "SplitVertical" | "pane::SplitVertical" | "pane::SplitRight" => {
                app(Command::SplitRight)
            }
            "SplitHorizontal" | "pane::SplitHorizontal" | "pane::SplitDown" => {
                app(Command::SplitDown)
            }
            "ActivatePaneLeft" | "workspace::ActivatePaneLeft" => app(Command::FocusLeft),
            "ActivatePaneRight" | "workspace::ActivatePaneRight" => app(Command::FocusRight),
            "ActivatePaneUp" | "workspace::ActivatePaneUp" => app(Command::FocusUp),
            "ActivatePaneDown" | "workspace::ActivatePaneDown" => app(Command::FocusDown),
            "ActivateNextItem" | "pane::ActivateNextItem" => app(Command::NextTab),
            "ActivatePreviousItem" | "pane::ActivatePreviousItem" => app(Command::PreviousTab),
            "ReopenClosedItem" | "pane::ReopenClosedItem" => app(Command::ReopenTab),
            "GoBack" | "pane::GoBack" => app(Command::GoBack),
            "GoForward" | "pane::GoForward" => app(Command::GoForward),
            "GoToDefinition" | "editor::GoToDefinition" => app(Command::GoToDefinition),
            "GoToTypeDefinition" | "editor::GoToTypeDefinition" => app(Command::GoToTypeDefinition),
            "GoToImplementation" | "editor::GoToImplementation" => app(Command::GoToImplementation),
            "GoToDeclaration" | "editor::GoToDeclaration" => app(Command::GoToDeclaration),
            "FindAllReferences" | "editor::FindAllReferences" => app(Command::FindReferences),
            "Hover" | "editor::Hover" => app(Command::Hover),
            "Rename" | "editor::Rename" => app(Command::Rename),
            "ToggleCodeActions" | "editor::ToggleCodeActions" => app(Command::CodeActions),
            "ShowCompletions" | "editor::ShowCompletions" => app(Command::ShowCompletions),
            "ShowSignatureHelp" | "editor::ShowSignatureHelp" => app(Command::ShowSignature),
            "GoToDiagnostic" | "editor::GoToDiagnostic" => app(Command::NextDiagnostic),
            "GoToPreviousDiagnostic" | "editor::GoToPreviousDiagnostic" => {
                app(Command::PreviousDiagnostic)
            }
            "GoToHunk" | "editor::GoToHunk" => app(Command::NextHunk),
            "GoToPreviousHunk" | "editor::GoToPreviousHunk" => app(Command::PreviousHunk),
            "ToggleFold" | "editor::ToggleFold" | "editor::Fold" | "editor::UnfoldLines" => {
                app(Command::ToggleFold)
            }
            "FoldAll" | "editor::FoldAll" => app(Command::FoldAll),
            "UnfoldAll" | "editor::UnfoldAll" => app(Command::UnfoldAll),
            "outline::Toggle" => app(Command::ShowSymbols),
            "project_symbols::Toggle" => app(Command::ShowWorkspaceSymbols),
            "file_finder::Toggle" => app(Command::ShowFiles),
            "pane::DeploySearch" => app(Command::SearchProject),
            "editor::MoveLineUp" => app(Command::MoveLineUp),
            "editor::MoveLineDown" => app(Command::MoveLineDown),
            "editor::SelectAllMatches" => app(Command::SelectAllMatches),
            "SelectNext" | "editor::SelectNext" => app(Command::SelectNext),
            "editor::Cancel" => app(Command::Cancel),
            _ => None,
        }
    }
}

/// A search for a character, as the character typed after `f` makes it.
pub(crate) fn find(forward: bool, till: bool, ch: char) -> Motion {
    Motion::Find(Find { ch, forward, till })
}
