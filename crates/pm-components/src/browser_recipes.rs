//! Recipes: browser scripts an agent saved by name with `browser_run`, listed beside the page so the reader can run one again without an agent, recorded, with the pass and fail of each of its checks in a table.

use crate::Button;
use crate::areas::BrowserEvent;
use crate::browser::{Browser, Command};
use crate::button::{self, Variant};
use crate::parts::{clipped, glyph};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::target::Target;
use crate::theme::{self, Role};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, Modifier};

/// How wide the panel of recipes is, in logical pixels.
pub const PANEL_WIDTH: f32 = 320.0;

/// What the pane asks the helper of the recipes, through the `browser_run` tool on the tab shown.
#[derive(Clone, Debug, PartialEq)]
pub enum RecipeAction {
    /// List the saved recipes.
    List,
    /// Run the recipe of this name, recorded.
    Run(String),
}

/// A saved recipe, as the panel lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Its name.
    pub name: String,
    /// The first line of its script.
    pub first: String,
}

/// One check a run made: a row of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeCheck {
    /// What the script called it.
    pub name: String,
    /// Whether it passed.
    pub ok: bool,
    /// What the script gave with it, as text.
    pub detail: String,
}

/// How the last run of a recipe from the pane went.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeRun {
    /// The recipe's name.
    pub name: String,
    /// Whether it passed: no error and no failed check.
    pub ok: bool,
    /// Why it stopped, when it threw or could not run.
    pub error: Option<String>,
    /// Its checks, in order.
    pub checks: Vec<RecipeCheck>,
    /// How many steps it took.
    pub steps: u64,
    /// How long it ran, in milliseconds.
    pub ms: u64,
    /// The folder its recording went into.
    pub recording: Option<String>,
}

/// The recipes the panel lists and how the last run went.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recipes {
    /// The saved recipes, newest first, as the helper last listed them.
    pub list: Vec<Recipe>,
    /// Whether the panel is open beside the page.
    pub open: bool,
    /// The recipe running, while one does.
    pub running: Option<String>,
    /// How the last run went.
    pub last: Option<RecipeRun>,
    /// Why the recipes could not be listed, when they could not.
    pub note: Option<String>,
    /// How far the panel is scrolled.
    pub scroll: rustle::ScrollState,
}

impl Recipes {
    /// Takes in what the helper answered `action` with: `browser_run`'s JSON, or why it failed.
    pub fn answered(&mut self, action: &RecipeAction, text: &str) {
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
        match action {
            RecipeAction::List => {
                let Some(found) = parsed["recipes"].as_array() else {
                    self.note = Some(text.to_owned());
                    return;
                };
                self.note = None;
                self.list = found
                    .iter()
                    .map(|recipe| Recipe {
                        name: recipe["name"].as_str().unwrap_or_default().to_owned(),
                        first: recipe["first"].as_str().unwrap_or_default().to_owned(),
                    })
                    .collect();
            }
            RecipeAction::Run(name) => {
                self.running = None;
                let error = match parsed["error"].as_str() {
                    Some(error) => Some(error.to_owned()),
                    None if parsed.is_object() => None,
                    None => Some(text.to_owned()),
                };
                let checks = parsed["checks"]
                    .as_array()
                    .map(|checks| {
                        checks
                            .iter()
                            .map(|check| RecipeCheck {
                                name: check["name"].as_str().unwrap_or_default().to_owned(),
                                ok: check["ok"].as_bool().unwrap_or(false),
                                detail: check["detail"].as_str().unwrap_or_default().to_owned(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.last = Some(RecipeRun {
                    name: name.clone(),
                    ok: parsed["ok"].as_bool().unwrap_or(false),
                    error,
                    checks,
                    steps: parsed["steps"].as_u64().unwrap_or(0),
                    ms: parsed["ms"].as_u64().unwrap_or(0),
                    recording: parsed["recording"].as_str().map(str::to_owned),
                });
            }
        }
    }
}

impl WorkspaceState {
    /// Takes in what the helper answered a recipe `action` of the pane with, in the browser that holds the tab it ran on.
    pub fn browser_recipe_answered(&mut self, action: &RecipeAction, text: &str) {
        if let Some(browser) = self.browser.as_mut() {
            browser.recipes.answered(action, text);
        }
    }

    /// Carries out what pressing `id` of the panel of recipes, or the button opening it, does; says whether `id` was one of them.
    pub(crate) fn browser_recipes_pressed(&mut self, id: Target) -> bool {
        let Target::Browser(event) = id else {
            return false;
        };
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        let recipes = &mut browser.recipes;
        let command = match event {
            BrowserEvent::BrowserRecipes => {
                recipes.open = !recipes.open;
                recipes.open.then_some(RecipeAction::List)
            }
            BrowserEvent::BrowserRecipesRefresh => Some(RecipeAction::List),
            BrowserEvent::BrowserRecipeRun(at) if recipes.running.is_none() => {
                let Some(recipe) = recipes.list.get(at) else {
                    return true;
                };
                recipes.running = Some(recipe.name.clone());
                Some(RecipeAction::Run(recipe.name.clone()))
            }
            BrowserEvent::BrowserRecipeRun(_) => None,
            _ => return false,
        };
        if let Some(action) = command {
            self.browser_queue(Command::Recipe(action));
        }
        true
    }
}

/// Emits the button of the toolbar that opens and closes the panel of recipes.
pub(crate) fn recipes_button(browser: &Browser) {
    Button::icon(
        phosphor::LIST_CHECKS,
        Target::Browser(BrowserEvent::BrowserRecipes),
    )
    .variant(if browser.recipes.open {
        Variant::Secondary
    } else {
        Variant::Ghost
    })
    .tooltip("Recipes: browser scripts agents saved, to run again")
    .build();
}

/// Emits text of `pixels` size in `tone`, at most `most` lines, ending in an ellipsis when cut.
fn lines(content: &str, pixels: u32, tone: Role, most: usize) {
    rustle::basic_text(
        Modifier::new().fill_max_width(),
        content,
        theme::line(pixels, tone).max_lines(most),
        rustle::TextOverflow::Ellipsis,
    );
}

/// Emits the mark of whether a check passed.
fn mark(ok: bool) {
    if ok {
        glyph(phosphor::CHECK_CIRCLE, 3.5, Role::Success);
    } else {
        glyph(phosphor::X_CIRCLE, 3.5, Role::Danger);
    }
}

/// Emits a thin line across the panel.
fn rule() {
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .height(1.0)
            .background(theme::color(Role::Border)),
        Arrangement::Start,
        Alignment::Start,
        || {},
    );
}

/// Emits one recipe of the list: its name, the first line of its script, and the button that runs it.
#[track_caller]
fn recipe_row(recipe: &Recipe, at: usize, running: Option<&str>) {
    rustle::row_arranged(
        Modifier::new().fill_max_width().padding(theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            rustle::column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    lines(&recipe.name, 12, Role::Text, 1);
                    lines(&recipe.first, 11, Role::Muted, 1);
                },
            );
            let this = running == Some(recipe.name.as_str());
            button::small_button(
                if this { "Running" } else { "Run" },
                Target::Browser(BrowserEvent::BrowserRecipeRun(at)),
                Variant::Secondary,
            )
            .disabled(running.is_some())
            .build();
        },
    );
}

/// Emits how the last run went: its verdict, then a row for each check with its mark, name and detail.
#[track_caller]
fn run_table(run: &RecipeRun) {
    rustle::row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            mark(run.ok);
            clipped(
                format!("{} {}", run.name, if run.ok { "passed" } else { "failed" }),
                12,
                Role::Text,
            );
        },
    );
    let passed = run.checks.iter().filter(|check| check.ok).count();
    lines(
        &format!(
            "{passed} of {} checks passed · {} steps · {:.1} s",
            run.checks.len(),
            run.steps,
            run.ms as f64 / 1000.0
        ),
        11,
        Role::Muted,
        1,
    );
    if let Some(error) = &run.error {
        lines(error, 11, Role::Danger, 3);
    }
    for (at, check) in run.checks.iter().enumerate() {
        rustle::key(at, || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Start,
                || {
                    mark(check.ok);
                    rustle::column_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            lines(&check.name, 11, Role::Text, 2);
                            if !check.detail.is_empty() {
                                lines(&check.detail, 11, Role::Muted, 2);
                            }
                        },
                    );
                },
            );
        });
    }
    if let Some(folder) = &run.recording {
        lines(&format!("Recorded in {folder}"), 11, Role::Muted, 2);
    }
}

/// Emits the panel of recipes beside the page, as tall as its row and [`PANEL_WIDTH`] wide: a heading, the table of the last run and the list of recipes; nothing while it is closed.
#[track_caller]
pub(crate) fn recipes_panel(recipes: &Recipes) {
    if !recipes.open {
        return;
    }
    rustle::column_arranged(
        Modifier::new()
            .width(1.0)
            .fill_max_height()
            .background(theme::color(Role::Border)),
        Arrangement::Start,
        Alignment::Start,
        || {},
    );
    rustle::column_arranged(
        Modifier::new()
            .width(PANEL_WIDTH)
            .fill_max_height()
            .background(theme::color(Role::Surface))
            .padding(theme::sp(2))
            .vertical_scroll(&recipes.scroll),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Center,
                || {
                    rustle::row_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Center,
                        || clipped("Recipes", 13, Role::Text),
                    );
                    Button::icon(
                        phosphor::ARROW_CLOCKWISE,
                        Target::Browser(BrowserEvent::BrowserRecipesRefresh),
                    )
                    .size(Size::Xs)
                    .tooltip("List the recipes again")
                    .build();
                    Button::icon(phosphor::X, Target::Browser(BrowserEvent::BrowserRecipes))
                        .size(Size::Xs)
                        .build();
                },
            );
            if let Some(run) = &recipes.last {
                run_table(run);
                rule();
            }
            if let Some(note) = &recipes.note {
                lines(note, 11, Role::Danger, 3);
            } else if recipes.list.is_empty() {
                lines(
                    "No recipes yet. An agent saves one with browser_run and save.",
                    11,
                    Role::Muted,
                    3,
                );
            }
            let running = recipes.running.as_deref();
            for (at, recipe) in recipes.list.iter().enumerate() {
                rustle::key(recipe.name.as_str(), || recipe_row(recipe, at, running));
            }
        },
    );
}

#[cfg(test)]
mod tests;
