//! The settings pane: a tree of pages and their sections down the left, and
//! the page it has open, one section after another.
//!
//! Every row is the same shape, after Zed's: the name and what it does on
//! the left, the control on the right — or under them, for a control that
//! needs the width — and an undo mark beside the name while the value is
//! not the one a first launch starts from. A switch flips, a number steps,
//! and a font or a colour is asked for in the picker.

use std::path::PathBuf;

use pm_gfx::Rgba;
use pm_ui::{
    Appearance, Div, Element, IconName, IconSize, IntoElement, Styled, Theme, button, h_flex, icon,
    icon_button, rule, scroll_area, space, switch, text, theme_gallery, toggle_grid, toggle_row,
    v_flex,
};

use crate::config::{
    FontSlot, InstallLanguageServers, Preference, Preferences, Step, ThemeMode, WorktreePaths,
};
use crate::editor::CursorShape;
use crate::keymap::{Action, Keymap};
use crate::message::Message;
use crate::settings::state::{Recording, Settings, SettingsPage, SettingsSection, SettingsView};
use crate::theme::{self, Group, TOKENS, hex, in_group};
use crate::workspace::shortened;

/// Width of the sidebar listing the pages.
const SIDEBAR_WIDTH: f32 = 226.0;

/// Width a page is capped at, however wide the pane is.
const PAGE_WIDTH: f32 = 760.0;

/// Width of the theme mode toggle.
const MODE_WIDTH: f32 = 48.0;

/// Width of the button naming the port variable.
const PORT_WIDTH: f32 = 32.0;

/// Width of a toggle picking one of a few choices.
const CHOICE_WIDTH: f32 = 64.0;

/// Width of the button naming a font family.
const FONT_WIDTH: f32 = 48.0;

/// Width of the number between a stepper's two buttons.
const VALUE_WIDTH: f32 = 14.0;

/// Width of the button naming a colour.
const HEX_WIDTH: f32 = 26.0;

/// Width of the button naming the chords an action is pressed as.
const CHORDS_WIDTH: f32 = 64.0;

/// How many of an action's bindings its row names before it stops.
const CHORDS_SHOWN: usize = 2;

/// Side of the square a colour is shown in.
const SWATCH_SIZE: f32 = 20.0;

/// The columns a wrap guide can be drawn down, and none.
const WRAP_GUIDES: [Option<usize>; 4] = [None, Some(80), Some(100), Some(120)];

/// Diameter of the dot marking a page with something set on it.
const DOT_SIZE: f32 = 6.0;

/// How strongly the line joining a page's sections is drawn.
const TREE_LINE_ALPHA: f32 = 0.5;

/// How strongly the outline of the lit entry of the tree is drawn.
const LIT_BORDER_ALPHA: f32 = 0.4;

/// What the settings pane is drawn from.
pub struct SettingsPane<'a> {
    /// Which page is open, and how far down it.
    pub settings: &'a Settings,
    /// The preferences the pane edits.
    pub preferences: &'a Preferences,
    /// The keymap in force, which the keybindings are read from.
    pub keymap: &'a Keymap,
    /// The file the preferences are written to, when there is one.
    pub file: Option<PathBuf>,
}

/// Builds the settings pane in `theme`.
pub fn settings_pane(theme: &Theme, pane: &SettingsPane<'_>) -> Box<dyn Element<Message>> {
    let settings = pane.settings;
    let view = settings.view();
    let alone = view.sections().len() == 1;
    let sections = view
        .sections()
        .iter()
        .map(|section| {
            let rows = section_rows(theme, pane, *section);
            match alone {
                true => self::rows(theme, rows),
                false => self::section(theme, section.label(), rows),
            }
        })
        .collect::<Vec<_>>();

    Box::new(
        h_flex()
            .w_full()
            .flex_1()
            .items_stretch()
            .child(sidebar(theme, pane.preferences, settings))
            .child(v_flex().w_px(1.0).h_full().bg(theme.colors.border_variant))
            .child(
                scroll_area(
                    settings.scroll(),
                    v_flex()
                        .w_full()
                        .max_w_px(PAGE_WIDTH)
                        .mx_auto()
                        .px(8)
                        .py(6)
                        .gap(8)
                        .child(heading(theme, view, pane.file.as_ref()))
                        .children(sections),
                )
                .flex_1()
                .h_full(),
            ),
    )
}

/// The tree down the left of the pane: every page, and under each page of
/// several sections that is opened out, its sections.
fn sidebar(theme: &Theme, preferences: &Preferences, settings: &Settings) -> Div<Message> {
    let view = settings.view();
    let entries = SettingsPage::ALL.into_iter().flat_map(|page| {
        let expanded = page.has_sections() && settings.is_expanded(page);
        let children = page
            .sections()
            .iter()
            .filter(move |_| expanded)
            .map(move |section| {
                let lit = view == SettingsView::Section(*section);
                section_entry(theme, preferences, *section, lit)
            });
        let lit = view == SettingsView::Page(page);
        std::iter::once(page_entry(theme, preferences, page, expanded, lit)).chain(children)
    });

    v_flex()
        .w_px(SIDEBAR_WIDTH)
        .h_full()
        .p(2.5)
        .gap(0.5)
        .bg(theme.colors.surface)
        .child(
            h_flex().h_px(theme.size.row).px(2).items_center().child(
                text("Settings")
                    .text_sm()
                    .font_semibold()
                    .color(theme.colors.text_muted),
            ),
        )
        .children(entries)
}

/// One page in the sidebar: the chevron that opens its sections out or
/// folds them away when it has several, its name, and a dot when something
/// on it is set.
fn page_entry(
    theme: &Theme,
    preferences: &Preferences,
    page: SettingsPage,
    expanded: bool,
    lit: bool,
) -> Div<Message> {
    let modified = page
        .sections()
        .iter()
        .any(|section| is_modified(preferences, *section));
    let chevron = match expanded {
        true => IconName::ChevronDown,
        false => IconName::ChevronRight,
    };
    let disclosure = match page.has_sections() {
        true => icon_button(theme, chevron, Message::ToggleSettingsPage(page))
            .tooltip(if expanded { "Collapse" } else { "Expand" }),
        false => h_flex().size_px(theme.size.icon_control),
    };

    tree_entry(theme, lit, Message::ShowSettingsPage(page))
        .child(disclosure)
        .child(tree_label(theme, page.label(), lit).flex_1())
        .when(modified, |entry| entry.child(dot(theme)))
}

/// One section under its page in the sidebar, hung off the line that
/// joins the page's sections, and a dot when something in it is set.
fn section_entry(
    theme: &Theme,
    preferences: &Preferences,
    section: SettingsSection,
    lit: bool,
) -> Div<Message> {
    tree_entry(theme, lit, Message::ShowSettingsSection(section))
        .child(
            h_flex()
                .w_px(theme.size.icon_control)
                .h_full()
                .justify_center()
                .child(
                    v_flex()
                        .w_px(1.0)
                        .h_full()
                        .bg(theme.colors.border.alpha(TREE_LINE_ALPHA)),
                ),
        )
        .child(tree_label(theme, section.label(), lit).flex_1())
        .when(is_modified(preferences, section), |entry| {
            entry.child(dot(theme))
        })
}

/// Whether anything `section` edits is set away from its default.
fn is_modified(preferences: &Preferences, section: SettingsSection) -> bool {
    section
        .preferences()
        .iter()
        .any(|preference| preferences.is_modified(*preference))
}

/// The dot marking an entry with something set in it.
fn dot(theme: &Theme) -> Div<Message> {
    v_flex()
        .size_px(DOT_SIZE)
        .rounded(DOT_SIZE / 2.0)
        .bg(theme.colors.text_muted)
}

/// A row of the sidebar's tree, washed while it is lit, that sends
/// `message` when it is pressed.
fn tree_entry(theme: &Theme, lit: bool, message: Message) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pl(0.5)
        .pr(2)
        .gap(1.5)
        .items_center()
        .rounded(theme.radius.sm)
        .border_1(match lit {
            true => theme.colors.border.alpha(LIT_BORDER_ALPHA),
            false => Rgba::TRANSPARENT,
        })
        .when(lit, |entry| entry.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(message)
}

/// The name of an entry of the sidebar's tree, quiet unless it is lit.
fn tree_label(theme: &Theme, label: &str, lit: bool) -> Div<Message> {
    let color = match lit {
        true => theme.colors.text,
        false => theme.colors.text_muted,
    };
    h_flex()
        .overflow_hidden()
        .child(text(label).text_sm().color(color))
}

/// What is shown — the page, or the section with the page it is on above
/// it — and where what it sets is written down.
fn heading(theme: &Theme, view: SettingsView, file: Option<&PathBuf>) -> Div<Message> {
    let title = match view {
        SettingsView::Page(page) => page.label(),
        SettingsView::Section(section) => section.label(),
    };
    v_flex()
        .w_full()
        .gap(1)
        .when_some(
            match view {
                SettingsView::Section(section) => Some(section.page().label()),
                SettingsView::Page(_) => None,
            },
            |heading, page| heading.child(text(page).text_sm().color(theme.colors.text_muted)),
        )
        .child(text(title).text_xl().font_semibold())
        .when_some(file, |heading, file| {
            heading.child(
                text(format!("Written to {}", shortened(file)))
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.text_subtle),
            )
        })
}

/// The rows of one section, top to bottom.
fn section_rows(
    theme: &Theme,
    pane: &SettingsPane<'_>,
    section: SettingsSection,
) -> Vec<Div<Message>> {
    let preferences = pane.preferences;
    let toggle =
        |preference, title, description| toggle(theme, preferences, preference, title, description);
    let stepper = |preference, title, description| {
        stepper(theme, preferences, preference, title, description)
    };
    match section {
        SettingsSection::Theme => vec![
            inline(
                theme,
                preferences,
                Preference::ThemeMode,
                "Mode",
                "Keep to one appearance, or follow the desktop's",
                theme_modes(preferences).w_px(space(MODE_WIDTH)),
            ),
            below(
                theme,
                preferences,
                Preference::ThemeFamily,
                "Family",
                "The family the editor is painted in; themes in the editor's home are listed too",
                theme_gallery(
                    theme::families(),
                    shown_appearance(theme, preferences),
                    preferences.theme_family,
                    Message::SetThemeFamily,
                ),
            ),
        ],
        SettingsSection::ThemeColors => theme_color_rows(theme, preferences),
        SettingsSection::Fonts => vec![
            group(
                theme,
                "Interface",
                vec![
                    font_row(
                        theme,
                        preferences,
                        FontSlot::Interface,
                        Preference::InterfaceFont,
                        "Family",
                        "The family labels, menus and prose are set in",
                    ),
                    stepper(
                        Preference::InterfaceFontSize,
                        "Size",
                        "The size body text is set at; every other size follows it",
                    ),
                ],
            ),
            group(
                theme,
                "Code",
                vec![
                    font_row(
                        theme,
                        preferences,
                        FontSlot::Buffer,
                        Preference::BufferFont,
                        "Family",
                        "The monospaced family files, paths and terminals are set in",
                    ),
                    stepper(
                        Preference::BufferFontSize,
                        "Size",
                        "The size a file is set at, before it is zoomed",
                    ),
                    stepper(
                        Preference::BufferFontWeight,
                        "Weight",
                        "How heavy code is set, from 100 to 900",
                    ),
                    stepper(
                        Preference::BufferLineHeight,
                        "Line Height",
                        "The distance between two lines, as a multiple of the size",
                    ),
                ],
            ),
        ],
        SettingsSection::Cursor => vec![
            inline(
                theme,
                preferences,
                Preference::CursorShape,
                "Shape",
                "How the caret is drawn",
                cursor_shapes(preferences).w_px(space(CHOICE_WIDTH)),
            ),
            toggle(
                Preference::CursorBlink,
                "Blink",
                "Blink the caret while its pane has the keyboard",
            ),
        ],
        SettingsSection::Indentation => vec![
            stepper(
                Preference::TabSize,
                "Tab Size",
                "How wide a step of indentation and a tab are, where a file does not say",
            ),
            toggle(
                Preference::HardTabs,
                "Hard Tabs",
                "Indent with tabs rather than spaces, where a file does not say",
            ),
        ],
        SettingsSection::Gutter => vec![
            toggle(
                Preference::LineNumbers,
                "Line Numbers",
                "Number the lines down the left of the text",
            ),
            toggle(
                Preference::RelativeLineNumbers,
                "Relative Line Numbers",
                "Count lines away from the cursor rather than from the top",
            ),
        ],
        SettingsSection::Highlighting => vec![
            toggle(
                Preference::CurrentLine,
                "Current Line",
                "Wash the line the cursor is on",
            ),
            toggle(
                Preference::Occurrences,
                "Occurrences",
                "Wash every other place the word at the cursor appears",
            ),
            toggle(
                Preference::IndentGuides,
                "Indent Guides",
                "Draw a line at every step of indentation",
            ),
            inline(
                theme,
                preferences,
                Preference::WrapGuide,
                "Wrap Guide",
                "Draw a line down the column lines are kept short of",
                wrap_guides(preferences).w_px(space(CHOICE_WIDTH)),
            ),
        ],
        SettingsSection::Display => vec![
            toggle(
                Preference::StickyScroll,
                "Sticky Scroll",
                "Keep the first lines of the blocks you are inside pinned above the text",
            ),
            toggle(
                Preference::Scrollbars,
                "Scrollbars",
                "Draw a scrollbar along each edge the text runs past",
            ),
            toggle(
                Preference::Minimap,
                "Minimap",
                "Draw the whole file in miniature down the right edge",
            ),
            toggle(
                Preference::Breadcrumbs,
                "Breadcrumbs",
                "Name the file's path and the blocks the cursor is inside above the text",
            ),
            toggle(
                Preference::SplitDiff,
                "Side-by-Side Diff",
                "Set the old and the new side of a diff beside each other",
            ),
            toggle(
                Preference::InlayHints,
                "Inlay Hints",
                "Let a language server write types and parameter names into the lines",
            ),
            toggle(
                Preference::CodeLens,
                "Code Lens",
                "Let a language server write counts of uses and ways to run a declaration after it",
            ),
            stepper(
                Preference::ScrollSensitivity,
                "Scroll Sensitivity",
                "How far a notch of the wheel scrolls, against its usual distance",
            ),
        ],
        SettingsSection::Saving => vec![
            inline(
                theme,
                preferences,
                Preference::InstallLanguageServers,
                "Install Language Servers",
                "How to handle a missing language server",
                install_language_servers(preferences).w_px(space(CHOICE_WIDTH)),
            ),
            toggle(
                Preference::FormatOnSave,
                "Format on Save",
                "Lay a file out the way its formatter would every time it is written",
            ),
            toggle(
                Preference::TrimWhitespace,
                "Remove Trailing Whitespace",
                "Take the spaces and tabs off the ends of lines when a file is saved",
            ),
            toggle(
                Preference::FinalNewline,
                "Ensure Final Newline",
                "End a file with a line break when it is saved",
            ),
        ],
        SettingsSection::Keymap => vec![
            below(
                theme,
                preferences,
                Preference::Keymap,
                "Base Keymap",
                "Keep the bindings your hands already know; keymaps in the editor's home are listed too",
                toggle_grid(crate::config::keymap_choices(), Some(preferences.keymap), 4),
            ),
            toggle(
                Preference::VimMode,
                "Vim Mode",
                "Modal editing, built in rather than an extension",
            ),
            inline(
                theme,
                preferences,
                Preference::VimClipboard,
                "System Clipboard",
                "When vim's yanks and deletes go to the system clipboard",
                vim_clipboards(preferences).w_px(space(CHOICE_WIDTH)),
            ),
            keymap_folder_row(theme),
        ],
        SettingsSection::Keybindings => keybinding_rows(theme, pane),
        SettingsSection::Terminal => vec![
            stepper(
                Preference::TerminalFontSize,
                "Font Size",
                "The size a terminal is set at, in the family code is set in",
            ),
            stepper(
                Preference::TerminalScrollback,
                "Scrollback",
                "How many lines a terminal keeps behind its screen",
            ),
        ],
        SettingsSection::Sessions => std::iter::once(toggle(
            Preference::TrustWorktrees,
            "Trust New Worktrees",
            "Run language servers and tasks in a session's worktree without asking first",
        ))
        .chain(bootstrap_rows(theme, preferences))
        .collect(),
    }
}

/// Every colour of the theme being drawn, a group at a time under what
/// they repaint, each one repaintable over the appearance in front.
fn theme_color_rows(theme: &Theme, preferences: &Preferences) -> Vec<Div<Message>> {
    let appearance = theme.appearance;
    let side = match appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    };
    let family = theme::family(preferences.theme_family).name;
    let summary = inline(
        theme,
        preferences,
        Preference::ThemeOverrides(appearance),
        &format!("Repainting {family}, {side}"),
        &format!(
            "A colour set here repaints every family's {side} variant, until it is saved as a theme of your own"
        ),
        button("Save as Theme…", Message::SaveTheme).outlined(),
    );

    let folder = crate::config::themes_directory()
        .map(|folder| shortened(&folder))
        .unwrap_or_default();
    let reload = action(
        theme,
        "Reload Themes",
        &format!("Read the themes in {folder} again, after editing one by hand"),
        button("Reload", Message::ReloadThemes).outlined(),
    );

    std::iter::once(summary)
        .chain(Group::ALL.into_iter().map(|group| {
            let colors = in_group(group)
                .map(|token| color_row(theme, preferences, appearance, token))
                .collect();
            self::group(theme, group.label(), colors)
        }))
        .chain(std::iter::once(reload))
        .collect()
}

/// The row that reads the keymaps in the editor's home again.
fn keymap_folder_row(theme: &Theme) -> Div<Message> {
    let folder = crate::config::keymaps_directory()
        .map(|folder| shortened(&folder))
        .unwrap_or_default();
    action(
        theme,
        "Reload Keymaps",
        &format!("Read the keymaps in {folder} again, after editing one by hand"),
        button("Reload", Message::ReloadKeymaps).outlined(),
    )
}

/// Every action, a group at a time, with the chords it is pressed as and
/// the way to press it as something else.
fn keybinding_rows(theme: &Theme, pane: &SettingsPane<'_>) -> Vec<Div<Message>> {
    let preferences = pane.preferences;
    let keymap = crate::keymap::name(preferences.keymap);
    let summary = inline(
        theme,
        preferences,
        Preference::Keybindings,
        &format!("Changing {keymap}"),
        "Click a binding and press the chords for it; Enter keeps them, Escape lets go, Backspace takes one back",
        button("Save as Keymap…", Message::SaveKeymap).outlined(),
    );
    let recording = pane.settings.recording();
    let mut groups: Vec<(&str, Vec<Div<Message>>)> = Vec::new();
    for action in Action::all() {
        let row = binding_row(theme, preferences, pane.keymap, recording, action);
        match groups.last_mut() {
            Some((group, rows)) if *group == action.group() => rows.push(row),
            _ => groups.push((action.group(), vec![row])),
        }
    }
    std::iter::once(summary)
        .chain(
            groups
                .into_iter()
                .map(|(label, rows)| self::group(theme, label, rows)),
        )
        .collect()
}

/// One action: its title and name, the undo mark while the reader has
/// changed its bindings, and the chords it is pressed as.
fn binding_row(
    theme: &Theme,
    preferences: &Preferences,
    keymap: &Keymap,
    recording: Option<&Recording>,
    action: Action,
) -> Div<Message> {
    let bound = keymap.bindings_of(action);
    let listening = recording.filter(|recording| recording.action == action);
    let label = match listening {
        Some(recording) if recording.chords.is_empty() => "Press keys…".to_owned(),
        Some(recording) => recording
            .sequence()
            .map_or_else(String::new, |sequence| format!("{sequence} …")),
        None if bound.is_empty() => "Unbound".to_owned(),
        None => bound
            .iter()
            .take(CHORDS_SHOWN)
            .map(|binding| binding.sequence.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    };
    let chords = button(label, Message::RecordBinding(action));
    let chords = match listening {
        Some(_) => chords,
        None => chords.outlined(),
    };
    h_flex()
        .w_full()
        .py(1.5)
        .gap(6)
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .h_px(theme.size.icon_control)
                .gap(2)
                .items_center()
                .child(text(action.title()).text_sm())
                .child(text(action.id()).text_sm().color(theme.colors.text_muted))
                .when_some(
                    reset_mark(theme, preferences, Preference::Binding(action)),
                    Div::child,
                ),
        )
        .child(
            h_flex()
                .gap(1)
                .items_center()
                .when(!bound.is_empty() && listening.is_none(), |row| {
                    row.child(
                        icon_button(theme, IconName::Close, Message::UnbindAction(action))
                            .tooltip("Remove Binding"),
                    )
                })
                .child(chords.w_px(space(CHORDS_WIDTH))),
        )
}

/// A run of rows under a small heading of their own, inside a section.
fn group(theme: &Theme, label: &str, rows: Vec<Div<Message>>) -> Div<Message> {
    v_flex()
        .w_full()
        .pt(3)
        .child(
            text(label)
                .text_sm()
                .font_medium()
                .color(theme.colors.text_muted),
        )
        .child(self::rows(theme, rows))
}

/// One colour: its name, the undo mark while it is repainted, and the
/// colour itself with the way to repaint it.
fn color_row(
    theme: &Theme,
    preferences: &Preferences,
    appearance: Appearance,
    token: usize,
) -> Div<Message> {
    let color = TOKENS[token].read(theme);
    let preference = Preference::ThemeColor(appearance, token);
    h_flex()
        .w_full()
        .py(1.5)
        .gap(6)
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .h_px(theme.size.icon_control)
                .gap(1)
                .items_center()
                .child(text(TOKENS[token].label()).text_sm())
                .when_some(reset_mark(theme, preferences, preference), Div::child),
        )
        .child(
            h_flex()
                .gap(2)
                .items_center()
                .child(
                    v_flex()
                        .size_px(SWATCH_SIZE)
                        .rounded(theme.radius.sm)
                        .border_1(theme.colors.border)
                        .bg(color),
                )
                .child(
                    button(hex(color), Message::EditThemeColor(token))
                        .outlined()
                        .w_px(space(HEX_WIDTH)),
                ),
        )
}

/// What a fresh worktree is given: the paths linked and copied into it,
/// and the variable its port is handed in.
fn bootstrap_rows(theme: &Theme, preferences: &Preferences) -> Vec<Div<Message>> {
    vec![
        below(
            theme,
            preferences,
            Preference::WorktreeLink,
            "Linked In",
            "Symlinked from the repository into every new worktree, so they share one copy",
            path_list(theme, preferences, WorktreePaths::Linked),
        ),
        below(
            theme,
            preferences,
            Preference::WorktreeCopy,
            "Copied In",
            "Copied from the repository into every new worktree, so each can change its own",
            path_list(theme, preferences, WorktreePaths::Copied),
        ),
        inline(
            theme,
            preferences,
            Preference::WorktreePort,
            "Port Variable",
            "The variable a session's own port is handed to its programs in",
            button(
                preferences
                    .bootstrap
                    .port
                    .clone()
                    .unwrap_or_else(|| "None".to_owned()),
                Message::EditWorktreePort,
            )
            .outlined()
            .w_px(space(PORT_WIDTH)),
        ),
    ]
}

/// One list of paths a new worktree is given, each with a way off it, and
/// a way to add another.
fn path_list(theme: &Theme, preferences: &Preferences, list: WorktreePaths) -> Div<Message> {
    let paths = preferences.worktree_paths(list);
    let entries = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            h_flex()
                .w_full()
                .h_px(theme.size.row)
                .pl(2)
                .pr(1)
                .items_center()
                .justify_between()
                .rounded(theme.radius.md)
                .bg(theme.colors.surface)
                .child(text(path.display().to_string()).text_sm().font_mono())
                .child(
                    icon_button(
                        theme,
                        IconName::Close,
                        Message::RemoveWorktreePath(list, index),
                    )
                    .tooltip("Remove"),
                )
        })
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap(1)
        .when(paths.is_empty(), |list| {
            list.child(
                text("Nothing yet")
                    .text_sm()
                    .color(theme.colors.text_subtle),
            )
        })
        .children(entries)
        .child(
            h_flex()
                .w_fit()
                .h_px(theme.size.row)
                .px(2)
                .gap(1.5)
                .items_center()
                .rounded(theme.radius.md)
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(Message::AddWorktreePath(list))
                .child(
                    icon(IconName::Plus)
                        .size(IconSize::Small)
                        .color(theme.colors.text_muted),
                )
                .child(text("Add Path").text_sm().color(theme.colors.text_muted)),
        )
}

/// A heading over rows.
fn section(theme: &Theme, title: &str, rows: Vec<Div<Message>>) -> Div<Message> {
    v_flex()
        .w_full()
        .gap(1)
        .child(text(title).text_lg().font_semibold())
        .child(rule(theme))
        .child(self::rows(theme, rows))
}

/// Rows, with a hairline between each row and the next.
fn rows(theme: &Theme, rows: Vec<Div<Message>>) -> Div<Message> {
    let mut body = v_flex().w_full();
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            body = body.child(rule(theme));
        }
        body = body.child(row);
    }
    body
}

/// A row with a switch on the right, for a preference that is one.
fn toggle(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
) -> Div<Message> {
    inline(
        theme,
        preferences,
        preference,
        title,
        description,
        switch(
            preferences.flag(preference).unwrap_or_default(),
            Message::TogglePreference(preference),
        ),
    )
}

/// A row with a number on the right, and a step down and a step up
/// either side of it.
fn stepper(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
) -> Div<Message> {
    let value = preferences.number(preference).unwrap_or_default();
    inline(
        theme,
        preferences,
        preference,
        title,
        description,
        h_flex()
            .gap(1)
            .items_center()
            .child(
                icon_button(
                    theme,
                    IconName::Minus,
                    Message::StepPreference(preference, Step::Down),
                )
                .tooltip("Less"),
            )
            .child(
                h_flex()
                    .w_px(space(VALUE_WIDTH))
                    .justify_center()
                    .child(text(value).text_sm().font_mono()),
            )
            .child(
                icon_button(
                    theme,
                    IconName::Plus,
                    Message::StepPreference(preference, Step::Up),
                )
                .tooltip("More"),
            ),
    )
}

/// A row naming the family `slot` is set in, with the way to pick another.
fn font_row(
    theme: &Theme,
    preferences: &Preferences,
    slot: FontSlot,
    preference: Preference,
    title: &str,
    description: &str,
) -> Div<Message> {
    let family = preferences.fonts.family(slot).unwrap_or("Default");
    inline(
        theme,
        preferences,
        preference,
        title,
        description,
        button(family, Message::PickFont(slot))
            .outlined()
            .w_px(space(FONT_WIDTH)),
    )
}

/// A row that does something once rather than setting a preference.
fn action(
    theme: &Theme,
    title: &str,
    description: &str,
    control: impl IntoElement<Message>,
) -> Div<Message> {
    h_flex()
        .w_full()
        .py(3)
        .gap(6)
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .gap(0.5)
                .child(
                    h_flex()
                        .h_px(theme.size.icon_control)
                        .items_center()
                        .child(text(title).font_medium()),
                )
                .child(text(description).text_sm().color(theme.colors.text_muted)),
        )
        .child(control)
}

/// A row with its control on the right of its labels.
fn inline(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
    control: impl IntoElement<Message>,
) -> Div<Message> {
    h_flex()
        .w_full()
        .py(3)
        .gap(6)
        .items_center()
        .justify_between()
        .child(labels(theme, preferences, preference, title, description).flex_1())
        .child(control)
}

/// A row with its control under its labels, across the whole row.
fn below(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
    control: impl IntoElement<Message>,
) -> Div<Message> {
    v_flex()
        .w_full()
        .py(3)
        .gap(3)
        .child(labels(theme, preferences, preference, title, description))
        .child(control)
}

/// A row's name, the undo mark while it is set, and what it does.
fn labels(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
    title: &str,
    description: &str,
) -> Div<Message> {
    let reset = reset_mark(theme, preferences, preference);

    v_flex()
        .gap(0.5)
        .child(
            h_flex()
                .h_px(theme.size.icon_control)
                .gap(1)
                .items_center()
                .child(text(title).font_medium())
                .when_some(reset, Div::child),
        )
        .child(text(description).text_sm().color(theme.colors.text_muted))
}

/// The mark that puts `preference` back, while it is set away from its
/// default.
fn reset_mark(
    theme: &Theme,
    preferences: &Preferences,
    preference: Preference,
) -> Option<Div<Message>> {
    preferences.is_modified(preference).then(|| {
        icon_button(theme, IconName::Undo, Message::ResetPreference(preference))
            .tooltip("Reset to Default")
    })
}

/// The shapes the caret can be drawn in, with the chosen one lit.
fn cursor_shapes(preferences: &Preferences) -> Div<Message> {
    let selected = CursorShape::ALL
        .iter()
        .position(|shape| *shape == preferences.display.cursor_shape);
    toggle_row(
        CursorShape::ALL
            .into_iter()
            .map(|shape| (shape.label().to_owned(), Message::SetCursorShape(shape))),
        selected,
    )
}

/// How much vim's register shares with the clipboard, the chosen one lit.
fn vim_clipboards(preferences: &Preferences) -> Div<Message> {
    let selected = pm_vim::ClipboardUse::ALL
        .iter()
        .position(|sharing| *sharing == preferences.vim_clipboard);
    toggle_row(
        pm_vim::ClipboardUse::ALL.into_iter().map(|sharing| {
            (
                sharing.label().to_owned(),
                Message::SetVimClipboard(sharing),
            )
        }),
        selected,
    )
}

/// The columns a wrap guide can be drawn down, with the chosen one lit.
fn wrap_guides(preferences: &Preferences) -> Div<Message> {
    let selected = WRAP_GUIDES
        .iter()
        .position(|column| *column == preferences.display.wrap_guide);
    toggle_row(
        WRAP_GUIDES.into_iter().map(|column| {
            let label = column.map_or_else(|| "Off".to_owned(), |column| column.to_string());
            (label, Message::SetWrapGuide(column))
        }),
        selected,
    )
}

/// The theme modes, as a toggle with the chosen one lit.
fn theme_modes(preferences: &Preferences) -> Div<Message> {
    let selected = ThemeMode::ALL
        .iter()
        .position(|mode| *mode == preferences.theme_mode);
    toggle_row(
        ThemeMode::ALL
            .into_iter()
            .map(|mode| (mode.label().to_owned(), Message::SetThemeMode(mode))),
        selected,
    )
}

/// The appearance the theme previews are painted in, or none for both.
fn shown_appearance(theme: &Theme, preferences: &Preferences) -> Option<pm_ui::Appearance> {
    match preferences.theme_mode {
        ThemeMode::System => None,
        _ => Some(theme.appearance),
    }
}

/// The choices for installing missing language servers.
fn install_language_servers(preferences: &Preferences) -> Div<Message> {
    let selected = InstallLanguageServers::ALL
        .iter()
        .position(|mode| *mode == preferences.install_language_servers);
    toggle_row(
        InstallLanguageServers::ALL.into_iter().map(|mode| {
            (
                mode.label().to_owned(),
                Message::SetInstallLanguageServers(mode),
            )
        }),
        selected,
    )
}
