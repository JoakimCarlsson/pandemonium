//! Tests of the tools that let an agent start sessions.

use std::path::Path;
use std::process::Command;

use pm_harness::{Permission, Setting};
use serde_json::json;

use super::*;
use crate::core::Session;

/// Runs `git` with `arguments` in `folder` and panics when it fails.
fn git(folder: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(arguments)
        .current_dir(folder)
        .status()
        .expect("git");
    assert!(status.success(), "git {arguments:?} failed");
}

/// A repository with one commit on `main` under a fresh folder of `build/` named `name`, and a worktree of it on the branch `work`; returns the two folders.
fn repository(name: &str) -> (PathBuf, PathBuf) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(name);
    let _ = std::fs::remove_dir_all(&base);
    let repository = base.join("repository");
    std::fs::create_dir_all(&repository).expect("folder");
    let repository = repository.canonicalize().expect("folder");
    git(&repository, &["init", "-q", "-b", "main"]);
    std::fs::write(repository.join("file"), "one").expect("file");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-q", "-m", "first"]);
    let worktree = base.join("worktree");
    let path = worktree.to_string_lossy().into_owned();
    git(&repository, &["worktree", "add", "-q", "-b", "work", &path]);
    (repository, worktree)
}

/// The description of a session of the first agent that works in `directory` of the project `repository`.
fn meta_in(repository: &Path, directory: &Path) -> Meta {
    Meta {
        agent: pm_harness::agents()[0].id.to_owned(),
        repository: repository.to_string_lossy().into_owned(),
        directory: directory.to_string_lossy().into_owned(),
        title: "caller".to_owned(),
        ..Meta::default()
    }
}

/// A session described by `meta` that has done nothing.
fn session_of(meta: Meta) -> Session {
    Session::new(
        pm_harness::agents()[0],
        PathBuf::from(&meta.directory),
        Vec::new(),
        meta,
    )
}

/// The prompt is needed, the rest is optional, and changes are carried unless the agent says not to.
#[test]
fn a_prompt_is_needed_and_the_rest_has_defaults() {
    assert!(asked(&json!({})).is_err());
    assert!(asked(&json!({ "prompt": "  " })).is_err());
    let plain = asked(&json!({ "prompt": "Fix it" })).expect("asked");
    assert_eq!(plain.prompt, "Fix it");
    assert!(plain.carry);
    assert_eq!(plain.place, None);
    let full = asked(&json!({
        "prompt": "Fix it",
        "where": "chat",
        "base_branch": "main",
        "carry_changes": false,
        "container": true,
        "model": "opus",
    }))
    .expect("asked");
    assert_eq!(full.place.as_deref(), Some("chat"));
    assert_eq!(full.base.as_deref(), Some("main"));
    assert!(!full.carry);
    assert_eq!(full.container, Some(true));
    assert_eq!(full.model.as_deref(), Some("opus"));
}

/// A title is the prompt's first line, cut at a word when it is long.
#[test]
fn titles_come_from_the_first_line() {
    assert_eq!(
        title_from("\n  Fix the login loop\nMore"),
        "Fix the login loop"
    );
    let long = "Implement the settings page with every section the design names and more";
    let title = title_from(long);
    assert!(title.chars().count() <= TITLE_LENGTH);
    assert!(long.starts_with(&title));
    assert!(!title.ends_with(' '));
    assert_eq!(title_from(""), "Session");
}

/// A project is found by its folder or by its folder's name, and a name that fits none or several is refused with the list.
#[test]
fn projects_are_found_by_folder_or_name() {
    let projects = vec![
        "/work/alpha".to_owned(),
        "/work/beta".to_owned(),
        "/other/beta".to_owned(),
    ];
    assert_eq!(
        project_named("/work/alpha", &projects).unwrap(),
        "/work/alpha"
    );
    assert_eq!(project_named("ALPHA", &projects).unwrap(), "/work/alpha");
    assert!(
        project_named("beta", &projects)
            .unwrap_err()
            .contains("more than one")
    );
    assert!(
        project_named("gamma", &projects)
            .unwrap_err()
            .contains("/work/alpha")
    );
}

/// A model is named exactly, or by a part only one model has; the caller's other settings are kept.
#[test]
fn models_are_named_by_value_name_or_part() {
    let caller = Caller {
        settings: vec![
            ("model".to_owned(), "claude/sonnet".to_owned()),
            ("mode".to_owned(), "default".to_owned()),
        ],
        models: vec![
            ("claude/sonnet".to_owned(), "Sonnet".to_owned()),
            ("claude/opus".to_owned(), "Opus".to_owned()),
            ("codex/gpt".to_owned(), "GPT".to_owned()),
        ],
        ..Caller::default()
    };
    let with = |model: &str| Asked {
        prompt: "x".to_owned(),
        model: Some(model.to_owned()),
        ..Asked::default()
    };
    let settings = settings_for(&with("opus"), &caller).unwrap();
    assert!(settings.contains(&("model".to_owned(), "claude/opus".to_owned())));
    assert!(settings.contains(&("mode".to_owned(), "default".to_owned())));
    assert_eq!(settings.iter().filter(|(id, _)| id == "model").count(), 1);
    assert!(settings_for(&with("claude"), &caller).is_err());
    assert!(settings_for(&with("GPT"), &caller).is_ok());
    assert!(settings_for(&with("gemini"), &caller).is_err());
    let kept = settings_for(&Asked::default(), &caller).unwrap();
    assert_eq!(kept, caller.settings);
}

/// A worktree is made from what the caller has checked out, with its changes carried, unless a base is named; local works in the project's folder; a chat needs no project.
#[test]
fn plans_follow_where_the_caller_works() {
    let (repository, worktree) = repository("session-tools-plan");
    let caller = Caller {
        meta: meta_in(&repository, &worktree),
        ..Caller::default()
    };
    let ask = |extra: Value| {
        let mut arguments = json!({ "prompt": "Add the export button" });
        if let (Some(into), Some(from)) = (arguments.as_object_mut(), extra.as_object()) {
            into.extend(from.clone());
        }
        asked(&arguments).unwrap()
    };

    let default = plan(&ask(json!({})), &caller, &[]).unwrap();
    assert_eq!(default.place, Some(Place::Worktree));
    assert_eq!(default.base.as_deref(), Some("work"));
    assert_eq!(default.carry_from.as_deref(), Some(worktree.as_path()));
    assert_eq!(default.title, "Add the export button");

    let based = plan(&ask(json!({ "base_branch": "main" })), &caller, &[]).unwrap();
    assert_eq!(based.base.as_deref(), Some("main"));
    assert_eq!(based.carry_from, None);

    let bare = plan(&ask(json!({ "carry_changes": false })), &caller, &[]).unwrap();
    assert_eq!(bare.carry_from, None);

    let local = plan(&ask(json!({ "where": "local" })), &caller, &[]).unwrap();
    assert_eq!(local.directory, repository.to_string_lossy());
    assert_eq!(local.base, None);

    let chat = plan(&ask(json!({ "where": "chat" })), &Caller::default(), &[]).unwrap();
    assert_eq!(chat.place, Some(Place::Chat));
    assert!(chat.repository.is_empty());

    assert!(plan(&ask(json!({ "where": "elsewhere" })), &caller, &[]).is_err());
    let homeless = plan(&ask(json!({})), &Caller::default(), &["/p".to_owned()]);
    assert!(homeless.unwrap_err().contains("/p"));
    let _ = std::fs::remove_dir_all(repository.parent().unwrap());
}

/// A session is working, waiting for the reader, finished, stopped or failed as its log says.
#[test]
fn states_follow_the_log() {
    let prompted = Event::Prompted {
        text: "go".to_owned(),
        files: Vec::new(),
        checkpoint: false,
        turn: 1,
        saved: None,
    };
    let asking = Event::Ask(Permission {
        ticket: 1,
        title: "Run rm".to_owned(),
        command: Some("rm".to_owned()),
        plan: None,
        choices: Vec::new(),
    });
    let numbered = |events: Vec<Event>| -> Vec<(u64, Event)> {
        events
            .into_iter()
            .enumerate()
            .map(|(at, event)| (at as u64 + 1, event))
            .collect()
    };
    assert_eq!(
        state(true, true, &numbered(vec![prompted.clone()])),
        "working"
    );
    assert!(state(true, true, &numbered(vec![prompted.clone(), asking])).starts_with("waiting"));
    assert_eq!(
        state(
            false,
            true,
            &numbered(vec![prompted.clone(), Event::Finished(None)])
        ),
        "finished"
    );
    assert_eq!(
        state(
            false,
            true,
            &numbered(vec![
                prompted.clone(),
                Event::Finished(Some("stopped".to_owned()))
            ])
        ),
        "stopped: stopped"
    );
    assert_eq!(
        state(
            false,
            false,
            &numbered(vec![prompted, Event::Failed("gone".to_owned())])
        ),
        "failed: gone"
    );
    assert_eq!(state(false, false, &[]), "not started");
    assert_eq!(
        state(
            false,
            false,
            &numbered(vec![Event::Settings(Vec::<Setting>::new())])
        ),
        "not started"
    );
}

/// A long final message keeps its end.
#[test]
fn tails_keep_the_end() {
    assert_eq!(tail("short", 10), "short");
    assert_eq!(tail("abcdef", 3), "…def");
}

/// A session an agent started cannot start more, and a local session is refused while an agent runs in the project's folder.
#[test]
fn starting_is_refused_where_it_would_do_harm() {
    let (repository, worktree) = repository("session-tools-refused");
    let core = Core::new();
    let (caller, child, local) = (990_001, 990_002, 990_003);
    {
        let mut sessions = core.sessions.lock().unwrap();
        let mut busy = session_of(meta_in(&repository, &repository));
        busy.busy = true;
        sessions.insert(local, busy);
        sessions.insert(caller, session_of(meta_in(&repository, &worktree)));
        sessions.insert(
            child,
            session_of(Meta {
                started_by: Some(caller),
                ..meta_in(&repository, &worktree)
            }),
        );
    }
    let nested = start_for(&core, child, &json!({ "prompt": "More" })).unwrap_err();
    assert!(nested.contains("cannot start sessions"), "{nested}");
    let crowded = start_for(
        &core,
        caller,
        &json!({ "prompt": "Here", "where": "local" }),
    )
    .unwrap_err();
    assert!(crowded.contains(&format!("session {local}")), "{crowded}");
    assert_eq!(open_started_by(&core, caller), 1);
    let told = status(&core, caller, &json!({}));
    assert!(told.contains(&format!("## Session {child}")), "{told}");
    assert!(!told.contains(&format!("## Session {local}")), "{told}");
    let _ = std::fs::remove_dir_all(repository.parent().unwrap());
}

/// Both tools are offered, and `session_start` needs only a prompt.
#[test]
fn the_tools_are_offered() {
    let all = definitions();
    let names: Vec<&str> = all
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(names, ["session_start", "session_status"]);
    assert_eq!(all[0]["inputSchema"]["required"], json!(["prompt"]));
}
