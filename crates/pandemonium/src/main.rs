//! Application entry point: opens the window and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod keys;
mod notify;
mod scene;
mod snapshot;
mod view;
mod work;

use std::path::Path;

use winit::event_loop::{ControlFlow, EventLoop};

use app::App;
use snapshot::Shot;

/// Opens the window and runs the event loop until it is closed, or, given `--version`, prints the version
/// and leaves, or, given `--server`, runs the server
/// on this machine with the options that follow, or, given
/// `--snapshot <path> [elapsed_ms [x y [focus [light [screen]]]]]`, writes one frame to a PNG and leaves.
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--version") {
        println!("pandemonium {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if arguments.first().map(String::as_str) == Some("--server") {
        std::process::exit(pm_server::run_with_arguments(
            "pandemonium --server",
            arguments.into_iter().skip(1),
        ));
    }
    if arguments.first().map(String::as_str) == Some("--snapshot-browser") {
        let (Some(path), Some(url)) = (arguments.get(1), arguments.get(2)) else {
            eprintln!("pandemonium: --snapshot-browser needs a path and an address");
            std::process::exit(2);
        };
        let flag = |name: &str| arguments.iter().skip(3).any(|argument| argument == name);
        if let Err(error) =
            snapshot::write_browser(Path::new(path), url, flag("tools"), flag("input"))
        {
            eprintln!("pandemonium: {error}");
            std::process::exit(1);
        }
        return;
    }
    pm_update::forget_previous();
    if arguments.first().map(String::as_str) == Some("--snapshot") {
        let Some(path) = arguments.get(1) else {
            eprintln!("pandemonium: --snapshot needs a path");
            std::process::exit(2);
        };
        let elapsed_ms = arguments
            .get(2)
            .and_then(|value| value.parse().ok())
            .unwrap_or(700.0);
        let pointer = arguments
            .get(3)
            .zip(arguments.get(4))
            .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)));
        let focused = arguments
            .get(5)
            .and_then(|value| value.parse().ok())
            .map(pm_input::FocusId);
        let appearance = match arguments.get(6).map(String::as_str) {
            Some("light") => pm_theme::Appearance::Light,
            _ => pm_theme::Appearance::Dark,
        };
        let shot = match arguments.get(7).map(String::as_str) {
            Some("code") => Shot::Code,
            Some("codetsx") => Shot::CodeTsx,
            Some("codemdx") => Shot::CodeMdx,
            Some("codeenv") => Shot::CodeEnv,
            Some("codejs") => Shot::CodeJs,
            Some("codeshell") => Shot::CodeShell,
            Some("runcompose") => Shot::RunCompose,
            Some("rundockerfile") => Shot::RunDockerfile,
            Some("rungomod") => Shot::RunGoMod,
            Some("runpackage") => Shot::RunPackageJson,
            Some("codesample") => Shot::CodeSample,
            Some("redacted") => Shot::Redacted,
            Some("languageoffer") => Shot::LanguageOffer,
            Some("markdownpreview") => Shot::MarkdownPreview,
            Some("vectorpreview") => Shot::VectorPreview,
            Some("picturepreview") => Shot::PicturePreview,
            Some("softwrap") => Shot::SoftWrap,
            Some("completing") => Shot::Completing,
            Some("hovering") => Shot::Hovering,
            Some("debugging") => Shot::Debugging,
            Some("problems") => Shot::Problems,
            Some("sidebar-hidden") => Shot::SidebarHidden,
            Some("editormenu") => Shot::EditorMenu,
            Some("references") => Shot::References,
            Some("renaming") => Shot::Renaming,
            Some("codeactions") => Shot::CodeActions,
            Some("filehistory") => Shot::FileHistory,
            Some("find-in-selection") => Shot::FindInSelection,
            Some("find-in-selection-replaced") => Shot::FindInSelectionReplaced,
            Some("dragging-text") => Shot::DraggingText,
            Some("dragged-text") => Shot::DraggedText,
            Some("lightbulb") => Shot::Lightbulb,
            Some("ligatures") => Shot::Ligatures,
            Some("ligatures-off") => Shot::LigaturesOff,
            Some("bidi") => Shot::Bidi,
            Some("compare-clipboard") => Shot::CompareClipboard,
            Some("compare-saved") => Shot::CompareSaved,
            Some("compare-picking") => Shot::ComparePicking,
            Some("compare-file") => Shot::CompareFile,
            Some("timeline") => Shot::Timeline,
            Some("conflict") => Shot::Conflict,
            Some("diffsubmodule") => Shot::DiffSubmodule,
            Some("changesrepositories") => Shot::ChangesRepositories(false),
            Some("changesrepositoriespicked") => Shot::ChangesRepositories(true),
            Some("branchessubmodule") => Shot::BranchesSubmodule(false),
            Some("branchessubmoduleinside") => Shot::BranchesSubmodule(true),
            Some("signing") => Shot::Signing,
            Some("annotated") => Shot::Annotated,
            Some("color-swatches") => Shot::ColorSwatches,
            Some("color-picker") => Shot::ColorPicker,
            Some("vim") => Shot::Vim,
            Some("project-search") => Shot::ProjectSearch,
            Some("excerpts") => Shot::Excerpts,
            Some("finding") => Shot::Finding,
            Some("find-reopened") => Shot::FindReopened,
            Some("outline") => Shot::Outline,
            Some("cursors") => Shot::Cursors,
            Some("column-selection") => Shot::ColumnSelection,
            Some("folded") => Shot::Folded,
            Some("sticky") => Shot::Sticky,
            Some("finder") => Shot::Finder,
            Some("commands") => Shot::Commands,
            Some("tasks") => Shot::Tasks,
            Some("run-buttons") => Shot::RunButtons,
            Some("toolchains") => Shot::Toolchains,
            Some("sessions") => Shot::Sessions,
            Some("quickhelp") => Shot::QuickHelp,
            Some("themes") => Shot::Themes,
            Some("goline") => Shot::GoLine,
            Some("tabmenu") => Shot::TabMenu,
            Some("extensions") => Shot::Extensions,
            Some("extension-install") => Shot::ExtensionInstall,
            Some("composer-long") => Shot::ComposerLong,
            Some("signed-out") => Shot::SignedOut,
            Some("workspace") => Shot::Workspace,
            Some("session-states") => Shot::SessionStates(false),
            Some("session-states-unread") => Shot::SessionStates(true),
            Some("quitting") => Shot::Quitting,
            Some("update") => Shot::Update,
            Some("diff") => Shot::Diff,
            Some("worddiff") => Shot::WordDiff,
            Some("diff-listed") => Shot::DiffListed,
            Some("diff-compact") => Shot::DiffCompact,
            Some("commenting") => Shot::Commenting,
            Some("files") => Shot::Files,
            Some("files-large") => Shot::FilesLarge,
            Some("files-scrolled") => Shot::FilesScrolled,
            Some("input-stress") => Shot::InputStress,
            Some("input-stress-scrolled") => Shot::InputStressScrolled,
            Some("input-stress-composing") => Shot::InputStressComposing,
            Some("ignored-files") => Shot::IgnoredFiles,
            Some("source") => Shot::Source,
            Some("refused") => Shot::Refused,
            Some("sourcemenu") => Shot::SourceMenu,
            Some("historymenu") => Shot::HistoryMenu,
            Some("stashing") => Shot::Stashing,
            Some("changemenu") => Shot::ChangeMenu,
            Some("treemenu") => Shot::TreeMenu,
            Some("treepicked") => Shot::TreePicked,
            Some("naming") => Shot::Naming,
            Some("discarding") => Shot::Discarding,
            Some("branching") => Shot::Branching,
            Some("reading") => Shot::Reading,
            Some("editing") => Shot::Editing,
            Some("newfile") => Shot::NewFile,
            Some("terminal") => Shot::Terminal,
            Some("terminallink") => Shot::TerminalLink,
            Some("terminalmenu") => Shot::TerminalMenu,
            Some("browsermenu") => Shot::BrowserMenu,
            Some("elements") => Shot::Elements,
            Some("element-details") => Shot::ElementDetails,
            Some("element-scrolled") => Shot::ElementScrolled,
            Some("elements-sent") => Shot::ElementsSent,
            Some("split-started") => Shot::SplitStarted,
            Some("overview") => Shot::Overview,
            Some("submodules") => Shot::Submodules,
            Some("overviewdiff") => Shot::OverviewDiff,
            Some("overviewchangemenu") => Shot::OverviewChangeMenu,
            Some("resized") => Shot::Resized,
            Some("new") => Shot::New,
            Some("empty") => Shot::Empty,
            Some("history") => Shot::History,
            Some("history-scrolled") => Shot::HistoryScrolled,
            Some("projectmenu") => Shot::ProjectMenu,
            Some("dragging") => Shot::Dragging,
            Some("welcome") => Shot::Welcome,
            Some("welcometerminal") => Shot::WelcomeTerminal,
            Some("panes") => Shot::Panes,
            Some("panes-dragging") => Shot::PanesDragging,
            Some("pane-menu") => Shot::PaneMenu,
            Some("pane-split") => Shot::PaneSplit,
            Some("pane-browser") => Shot::PaneBrowser,
            Some("pane-split-file") => Shot::PaneSplitFile,
            Some("wrapped") => Shot::Wrapped,
            Some("slashed") => Shot::Slashed,
            Some("dollar") => Shot::Dollar,
            Some("dollar-session") => Shot::DollarSession,
            Some("scrolling") => Shot::Scrolling,
            Some("placing") => Shot::Placing,
            Some("live") => Shot::Live,
            Some("asking") => Shot::Asking,
            Some("asking-command") => Shot::AskingCommand,
            Some("inquiring") => Shot::Inquiring,
            Some("answered") => Shot::Answered,
            Some("hooks") => Shot::Hooks,
            Some("hooks-review") => Shot::HooksReview,
            Some("background") => Shot::Background,
            Some("subagents") => Shot::Agents,
            Some("subagents-expanded") => Shot::AgentsExpanded,
            Some("subagents-scrolled") => Shot::AgentsScrolled,
            Some("queued") => Shot::Queued,
            Some("usage") => Shot::Usage,
            Some("kept") => Shot::Kept,
            Some("restoring") => Shot::Restoring,
            Some("picking") => Shot::Picking,
            Some("expanded") => Shot::Expanded,
            Some("slashing") => Shot::Slashing,
            Some("missing") => Shot::Missing,
            Some("attached") => Shot::Attached,
            Some("unfolded") => Shot::Unfolded,
            Some("composing") => Shot::Composing,
            Some("devcontainer") => Shot::DevContainer,
            Some("terminalcomposing") => Shot::TerminalComposing,
            Some("selecting") => Shot::Selecting,
            Some("sessionaccount") => Shot::SessionAccount,
            Some("messagemenu") => Shot::MessageMenu,
            Some("adding") => Shot::Adding,
            Some("loading") => Shot::Loading,
            Some("owning") => Shot::Owning,
            Some("cloning") => Shot::Cloning,
            Some("folders") => Shot::Folders,
            Some(name) if Shot::settings_named(name).is_some() => {
                Shot::settings_named(name).unwrap_or(Shot::Empty)
            }
            _ => Shot::Empty,
        };
        if let Err(error) = snapshot::write(
            Path::new(path),
            elapsed_ms,
            pointer,
            focused,
            appearance,
            shot,
        ) {
            eprintln!("pandemonium: {error}");
            std::process::exit(1);
        }
        return;
    }

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("pandemonium: could not start the event loop: {error}");
            std::process::exit(1);
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let wake: pm_client::Wake = std::sync::Arc::new(move || {
        let _ = proxy.send_event(());
    });
    if let Err(error) = event_loop.run_app(&mut App::new(wake)) {
        eprintln!("pandemonium: the event loop failed: {error}");
        std::process::exit(1);
    }
}
