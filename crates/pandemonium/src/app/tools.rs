//! Opening, moving and drawing registered workspace tools in ordinary panes.

use pm_ui::{Axis, Div, MenuItem, Styled, Theme, menu_entry};
use std::collections::BTreeSet;

use crate::app::{App, Writing};
use crate::message::Message;
use crate::panel::{Panel, PanelView};
use crate::panes::{Arrangement, Item, PaneId, Role, SplitDirection, Tool};
use crate::review::SourceControlControls;
use crate::workspace::MenuTarget;
use pm_core::Scope;

/// Width restored for a column of tool tabs.
const DEFAULT_TOOL_WIDTH: f32 = 252.0;

/// Height restored for a row of tool tabs.
const DEFAULT_TOOL_HEIGHT: f32 = 220.0;

/// Space retained for the document group in a layout preset.
const DOCUMENT_REACH: f32 = 320.0;

impl App {
    /// Handles tool commands through the shared pane model.
    pub(super) fn tool_command(&mut self, message: Message) -> bool {
        match message {
            Message::ShowTool(tool) => self.show_tool(tool),
            Message::ToggleTool(tool) => match self.tool_pane(tool) {
                Some(pane) => self.close_item(pane, self.tool_item(tool)),
                None => self.show_tool(tool),
            },
            Message::ShowToolsMenu => self.open_menu(MenuTarget::Tools),
            Message::ResetWindowLayout => self.reset_window_layout(),
            _ => return false,
        }
        true
    }

    /// The tool identity for the worktree currently shown.
    pub(super) fn tool_item(&self, tool: Tool) -> Item {
        Item::tool(tool, self.scope())
    }

    /// The pane holding the registered tool, including one whose tab is behind another.
    pub(super) fn tool_pane(&self, tool: Tool) -> Option<PaneId> {
        self.panes.panes().into_iter().find(|id| {
            self.panes
                .pane(*id)
                .is_some_and(|pane| pane.tabs(self.scope()).contains(&self.tool_item(tool)))
        })
    }

    /// Whether a tool is the front tab of any pane currently drawn.
    pub(super) fn tool_visible(&self, tool: Tool) -> bool {
        self.panes.panes().into_iter().any(|id| {
            self.panes
                .pane(id)
                .and_then(|pane| pane.active(self.scope()))
                == Some(self.tool_item(tool))
        })
    }

    /// Brings a tool forward, keeping the chat launcher in the focused conversation's pane.
    pub(super) fn show_tool(&mut self, tool: Tool) {
        let pane = if tool == Tool::Chat && matches!(self.active_tab(), Some(Item::Agent(_, _))) {
            self.panes.focus()
        } else {
            match self.tool_pane(tool) {
                Some(pane) => pane,
                None => self.pane_for(self.panes.focus(), self.tool_item(tool).role()),
            }
        };
        self.move_tool(pane, tool);
    }

    /// Moves the current worktree's tool tab to `target`, preserving panes and other tabs.
    fn move_tool(&mut self, target: PaneId, tool: Tool) {
        if self.panes.pane(target).is_none() {
            return;
        }
        let item = self.tool_item(tool);
        let scope = self.scope();
        if let Some(source) = self.tool_pane(tool)
            && source != target
        {
            let tab = self.panes.pane_mut(source).and_then(|pane| pane.take(item));
            if let (Some(tab), Some(pane)) = (tab, self.panes.pane_mut(target)) {
                pane.append(tab, scope);
            }
        } else if let Some(pane) = self.panes.pane_mut(target) {
            pane.open(scope, item);
        }
        self.activate_tab(target, item);
    }

    /// Menu entries opening registered workspace tools.
    pub(super) fn tool_menu(&self) -> Vec<MenuItem<Message>> {
        Tool::ALL
            .into_iter()
            .map(|tool| {
                menu_entry(
                    format!("Open {}", tool.label()),
                    Some(Message::ShowTool(tool)),
                )
            })
            .collect()
    }

    /// The window's width and height, from the surface or the last size written down.
    fn window_size(&self) -> pm_gfx::Size {
        self.renderer
            .as_ref()
            .map(|renderer| renderer.size())
            .unwrap_or(pm_gfx::Size::new(
                self.window_state.width,
                self.window_state.height,
            ))
    }

    /// The default division of one project's window, and the index of the
    /// pane its documents go in.
    ///
    /// Projects, files and changes share the left column, documents have the
    /// middle with the terminal beneath them, and chat has the right column.
    /// Any other tool in `held` keeps to the side it opens on, conversations in
    /// `documents` join chat, and the rest are the tabs the document pane starts with.
    pub(super) fn default_arrangement(
        &self,
        scope: Option<Scope>,
        documents: Vec<Item>,
        held: &BTreeSet<Item>,
    ) -> (Arrangement, usize) {
        let group = |direction| {
            let mut items = Vec::new();
            for tool in Tool::ALL
                .into_iter()
                .filter(|tool| tool.default_split() == direction)
            {
                let opened = tool.opens_by_default().then(|| Item::tool(tool, scope));
                let kept = held.iter().copied().filter(|item| {
                    matches!(item, Item::Tool(held) | Item::WorktreeTool(_, held) if *held == tool)
                });
                for item in opened.into_iter().chain(kept) {
                    if !items.contains(&item) {
                        items.push(item);
                    }
                }
            }
            items
        };
        let (conversations, documents): (Vec<_>, Vec<_>) = documents
            .into_iter()
            .partition(|item| item.role() == Role::Agent);
        let left = group(SplitDirection::Left);
        let mut right = group(SplitDirection::Right);
        right.extend(conversations);
        let above = group(SplitDirection::Up);
        let below = group(SplitDirection::Down);
        let size = self.window_size();
        let focus = usize::from(!left.is_empty()) + usize::from(!above.is_empty());
        let columns_width = DEFAULT_TOOL_WIDTH
            * (usize::from(!left.is_empty()) + usize::from(!right.is_empty())) as f32;
        let mut middle = Arrangement::Pane(documents);
        if !below.is_empty() {
            middle = Arrangement::Split {
                axis: Axis::Vertical,
                children: vec![
                    (
                        (size.height - DEFAULT_TOOL_HEIGHT).max(DOCUMENT_REACH),
                        middle,
                    ),
                    (DEFAULT_TOOL_HEIGHT, Arrangement::Pane(below)),
                ],
            };
        }
        if !above.is_empty() {
            middle = Arrangement::Split {
                axis: Axis::Vertical,
                children: vec![
                    (DEFAULT_TOOL_HEIGHT, Arrangement::Pane(above)),
                    (
                        (size.height - DEFAULT_TOOL_HEIGHT).max(DOCUMENT_REACH),
                        middle,
                    ),
                ],
            };
        }
        let mut children = Vec::new();
        if !left.is_empty() {
            children.push((DEFAULT_TOOL_WIDTH, Arrangement::Pane(left)));
        }
        children.push(((size.width - columns_width).max(DOCUMENT_REACH), middle));
        if !right.is_empty() {
            children.push((DEFAULT_TOOL_WIDTH, Arrangement::Pane(right)));
        }
        (
            Arrangement::Split {
                axis: Axis::Horizontal,
                children,
            },
            focus,
        )
    }

    /// Restores the active project's default tool groups while retaining its
    /// documents, buffers and running processes. No other project's window is touched.
    fn reset_window_layout(&mut self) {
        let scope = self.scope();
        let active = self
            .recent
            .get(&Role::Editor)
            .copied()
            .and_then(|id| self.panes.pane(id)?.active(scope));
        let fronts = self
            .panes
            .panes()
            .into_iter()
            .filter_map(|id| self.panes.pane(id)?.active(scope))
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        let documents = self
            .panes
            .panes()
            .into_iter()
            .flat_map(|id| {
                self.panes
                    .pane(id)
                    .map(|pane| pane.items().collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .filter(|item| {
                !matches!(item, Item::Tool(_) | Item::WorktreeTool(..)) && seen.insert(*item)
            })
            .collect::<Vec<_>>();
        let (layout, focus) = self.default_arrangement(scope, documents, &self.panes.held());
        self.panes.arrange(&layout, focus, scope);
        for id in self.panes.panes() {
            if let Some(pane) = self.panes.pane_mut(id) {
                let tabs = pane.tabs(scope);
                let front = tabs
                    .iter()
                    .copied()
                    .find(|item| fronts.contains(item))
                    .or_else(|| tabs.first().copied());
                if let Some(item) = front {
                    pane.activate(scope, item);
                }
            }
        }
        let pane = self.panes.focus();
        if let (Some(item), Some(pane)) = (active, self.panes.pane_mut(pane)) {
            pane.activate(scope, item);
        }
        self.drag = None;
        self.entry_drag = None;
        self.project_drag = None;
        if let Some(ui) = self.ui.as_mut() {
            let _ = ui.pointer_cancelled();
            ui.clear_text_selection();
        }
        self.recent.insert(Role::Editor, pane);
        self.release_pane_focus();
        self.focus_pane(pane);
        self.store();
    }

    /// Builds a registered tool's content independently of where its pane is placed.
    pub(super) fn tool_content(&self, theme: &Theme, tool: Tool, width: f32) -> Div<Message> {
        match tool {
            Tool::Projects => crate::workspace::projects_view(
                theme,
                &self.open,
                &self.sidebar_projects(),
                &self.project_groups,
                self.active_tab() == Some(Item::Tool(Tool::Projects)) && self.editor_focused,
                self.project_list.clone(),
            ),
            Tool::Files => {
                let listing = self.tree_listing();
                crate::tree::files_sidebar(theme, listing.as_ref(), width).w_full()
            }
            Tool::Changes => crate::review::changes_sidebar(
                theme,
                self.review(),
                self.writing == Some(Writing::Commit),
                width,
                SourceControlControls {
                    solid: self.caret_solid(),
                    commit_bounds: self.commit_bounds.clone(),
                    history_refs_bounds: self.history_refs_bounds.clone(),
                    history_graph_bounds: self.history_graph_bounds.clone(),
                    changes_area: self.changes_area.clone(),
                    history_all: self.history_all,
                    history_graph_height: self.history_graph.extent(),
                    history_graph_open: self.history_graph_open,
                    changes_section_open: self.changes_section_open,
                },
            )
            .w_full(),
            Tool::Chat => {
                let scope = self.scope();
                let context = scope.and_then(|scope| {
                    let project = self.open.get(scope.project())?;
                    Some(match scope.session().and_then(|id| self.sessions.get(id)) {
                        Some(session) => format!("{} · {}", project.name(), session.name()),
                        None => project.name().to_owned(),
                    })
                });
                let talks = self
                    .agents
                    .iter()
                    .filter(|talk| Some(talk.scope()) == scope)
                    .collect::<Vec<_>>();
                crate::agent::chat_pane(
                    theme,
                    context,
                    &self.agent_rows(),
                    &talks,
                    self.chat_scroll.clone(),
                )
            }
            Tool::Terminal => self.panel_content(theme, PanelView::Terminal),
            Tool::Problems => self.panel_content(theme, PanelView::Problems),
            Tool::Debug => self.panel_content(theme, PanelView::Debug),
        }
    }

    /// Builds a worktree panel view as content for its registered tool tab.
    fn panel_content(&self, theme: &Theme, view: PanelView) -> Div<Message> {
        let scope = self.scope();
        crate::panel::panel_content(
            theme,
            Panel {
                view,
                shell: scope.and_then(|scope| self.terminals.active(scope)),
                shells: scope.map_or_else(Vec::new, |scope| self.terminals.list(scope)),
                focused: self.terminal_focused,
                linking: self.modifiers.control_key(),
                problems: if view == PanelView::Problems {
                    self.problems()
                } else {
                    Vec::new()
                },
                problems_scroll: self.problems_scroll.clone(),
                problems_area: self.problems_area.clone(),
                debug: if view == PanelView::Debug {
                    self.debug_in_panel(theme)
                } else {
                    None
                },
            },
        )
    }

    /// The active worktree's file listing and the input state shared by its tool tab.
    fn tree_listing(&self) -> Option<crate::tree::Listing<'_>> {
        let scope = self.scope()?;
        Some(crate::tree::Listing {
            tree: self.files.get(&scope)?,
            review: self.reviews.get(&scope),
            selection: self.selections.get(&scope),
            edit: self.tree_edit.as_ref(),
            clipboard: self.tree_clipboard.as_ref(),
            dropping: self
                .entry_drag
                .as_ref()
                .filter(|drag| drag.is_carried())
                .and_then(|drag| drag.target.as_deref())
                .or(self.arriving.as_deref()),
            focused: self.tree_focused,
            caret: self.caret_solid(),
            scroll: self.tree_scrolls.get(&scope).cloned().unwrap_or_default(),
            rows: self.tree_rows.clone(),
            area: self.tree_area.clone(),
            field: self.tree_field.clone(),
        })
    }
}
