//! The bar offering to continue a chat, which belongs to no project, as a
//! session in a project the reader picks.

use std::rc::Rc;

use rustle::prelude::*;

use crate::button::{Button, ButtonVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// The visual treatment of the bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContinueChatVariant {
    /// A band across the top of the conversation.
    #[default]
    Band,
    /// A rounded card.
    Card,
}

/// The bar offering projects to continue the chat in.
#[derive(Clone)]
pub struct ContinueChat {
    /// The projects offered, by name.
    projects: Vec<String>,
    /// What picking a project does, given its position.
    on_pick: Option<Rc<dyn Fn(usize)>>,
    /// The project picked, by position, whose branches are offered.
    chosen: Option<usize>,
    /// The branches of the project picked, once its machine listed them; none of them for a folder that holds no repository.
    branches: Option<Vec<String>>,
    /// What picking the branch the session's worktree is made from does, given the branch.
    on_branch: Option<Rc<dyn Fn(String)>>,
    /// Whether the chat cannot be continued now, as while its agent works.
    disabled: bool,
    /// Its visual treatment.
    variant: ContinueChatVariant,
    /// Its text size.
    size: Size,
}

impl ContinueChat {
    /// A bar offering `projects`.
    pub fn new(projects: Vec<String>) -> Self {
        Self {
            projects,
            on_pick: None,
            chosen: None,
            branches: None,
            on_branch: None,
            disabled: false,
            variant: ContinueChatVariant::Band,
            size: Size::Xs,
        }
    }

    /// The bar running `on_pick` with the position of the project picked.
    pub fn on_pick(mut self, on_pick: impl Fn(usize) + 'static) -> Self {
        self.on_pick = Some(Rc::new(on_pick));
        self
    }

    /// The bar with the project at `chosen` picked, offering `branches` to make the session's worktree from once they are known.
    pub fn chosen(mut self, chosen: Option<usize>, branches: Option<Vec<String>>) -> Self {
        self.chosen = chosen;
        self.branches = branches;
        self
    }

    /// The bar running `on_branch` with the branch picked, which continues the chat in the project picked.
    pub fn on_branch(mut self, on_branch: impl Fn(String) + 'static) -> Self {
        self.on_branch = Some(Rc::new(on_branch));
        self
    }

    /// The bar with its button switched off while `disabled`, as while the agent works.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The bar with `variant`.
    pub fn variant(mut self, variant: ContinueChatVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The bar at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the bar.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let open = remember(|| false);
        let toggle = open.clone();
        let dismiss = open.clone();
        let modifier = match self.variant {
            ContinueChatVariant::Band => Modifier::new()
                .fill_max_width()
                .height(34.0)
                .border_bottom_hairline(scheme.outline_variant)
                .padding_xy(12.0, 0.0),
            ContinueChatVariant::Card => Modifier::new()
                .fill_max_width()
                .background_rounded(scheme.surface_container, 8.0)
                .padding_xy(12.0, 6.0),
        };
        row_arranged(
            modifier,
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Center,
            move || {
                icon(phosphor::CHAT_CIRCLE, 14.0, scheme.on_surface_variant);
                Label::new(if self.disabled {
                    "This chat belongs to no project. It can be continued in one once the agent is done."
                } else {
                    "This chat belongs to no project. Continue it as a session in one."
                })
                    .variant(LabelVariant::Muted)
                    .size(self.size)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                column_with(Modifier::new(), move || {
                    Button::new("Continue in project")
                        .icon(phosphor::FOLDER_SIMPLE)
                        .trailing_icon(phosphor::CARET_DOWN)
                        .variant(ButtonVariant::Secondary)
                        .size(Size::Xs)
                        .disabled(self.disabled)
                        .on_click(move || toggle.set(!toggle.get()))
                        .show();
                    let close = open.clone();
                    dropdown_menu(
                        open.get() && !self.disabled,
                        move || dismiss.set(false),
                        move || {
                            for (index, name) in self.projects.iter().enumerate() {
                                let on_pick = self.on_pick.clone();
                                let label = if self.chosen == Some(index) {
                                    format!("✓ {name}")
                                } else {
                                    name.clone()
                                };
                                dropdown_menu_item(&label, move || {
                                    if let Some(on_pick) = &on_pick {
                                        on_pick(index);
                                    }
                                });
                            }
                            if self.chosen.is_none() {
                                return;
                            }
                            match &self.branches {
                                None => dropdown_menu_item("Reading its branches…", || {}),
                                Some(branches) if branches.is_empty() => {
                                    let on_branch = self.on_branch.clone();
                                    let close = close.clone();
                                    dropdown_menu_item("Continue here", move || {
                                        close.set(false);
                                        if let Some(on_branch) = &on_branch {
                                            on_branch("HEAD".to_owned());
                                        }
                                    });
                                }
                                Some(branches) => {
                                    for branch in branches {
                                        let on_branch = self.on_branch.clone();
                                        let close = close.clone();
                                        let picked = branch.clone();
                                        dropdown_menu_item(
                                            &format!("Branch from {branch}"),
                                            move || {
                                                close.set(false);
                                                if let Some(on_branch) = &on_branch {
                                                    on_branch(picked.clone());
                                                }
                                            },
                                        );
                                    }
                                }
                            }
                        },
                    );
                });
            },
        );
    }
}
