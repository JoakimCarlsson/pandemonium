//! Tests of how the panel of recipes takes in what the helper answered.

use super::{RecipeAction, Recipes};

/// A listing fills the list, and a run of a recipe gives the table of its checks and ends the run.
#[test]
fn a_listing_and_a_run_fill_the_panel() {
    let mut recipes = Recipes::default();
    recipes.answered(
        &RecipeAction::List,
        r#"{"recipes":[{"name":"checkout-smoke","saved":1,"first":"await page.goto('/shop');"}]}"#,
    );
    assert_eq!(recipes.list.len(), 1);
    assert_eq!(recipes.list[0].name, "checkout-smoke");
    recipes.running = Some("checkout-smoke".to_owned());
    let run = RecipeAction::Run("checkout-smoke".to_owned());
    recipes.answered(
        &run,
        r#"{"ok":false,"value":202,"error":null,"checks":[{"name":"ingest 202","ok":true,"detail":""},{"name":"total","ok":false,"detail":"\"—\""}],"passed":1,"failed":1,"logs":[],"steps":4,"ms":1500,"recipe":"checkout-smoke","recording":"/r/one"}"#,
    );
    assert_eq!(recipes.running, None);
    let last = recipes.last.as_ref().expect("a run");
    assert!(!last.ok);
    assert_eq!(last.error, None);
    assert_eq!(last.checks.len(), 2);
    assert!(last.checks[0].ok && !last.checks[1].ok);
    assert_eq!(last.recording.as_deref(), Some("/r/one"));
}

/// An answer that is not the tool's JSON, such as a refusal, is shown as the reason.
#[test]
fn a_refusal_is_shown() {
    let mut recipes = Recipes::default();
    recipes.answered(&RecipeAction::List, "there is no tool browser_run");
    assert_eq!(
        recipes.note.as_deref(),
        Some("there is no tool browser_run")
    );
    recipes.answered(&RecipeAction::Run("x".to_owned()), "the browser stopped");
    assert_eq!(
        recipes.last.and_then(|run| run.error).as_deref(),
        Some("the browser stopped")
    );
}
