//! The window, built on Rustle and wired to the local server through
//! `pm-shell`'s link, with the settings window opened beside it.

use std::cell::RefCell;
use std::rc::Rc;

use pm_components::{AppTheme, KeyBindings, TerminalCommand, provide_key_bindings};
use pm_pages::{
    Actions, Destination, PICKER_SIZE, Panel, PickerActions, SETTINGS_SIZE, SettingsActions,
    SignInAction, UpdateAction, WindowAction, folder_picker_page, settings_page, workspace_page,
};
use pm_shell::{
    AddStep, BrowserAction, Link, Page, PickerAction, ProjectAction, QuickAction, QuickOutcome,
    SessionCommand, SettingsChange,
};
use rustle::prelude::*;
use rustle::{ResizeEdge, WindowHandle, current_window, set_external_handler};

/// Opens the Rustle window and runs it until it is closed.
pub fn run() {
    crate::performance::install();
    let mut connected = Link::connect(rustle::external_waker());
    let settings = connected.settings_mut();
    settings.editor = load_editor_settings();
    settings.window = load_window_settings();
    if settings.window.general.launch == pm_config::Launch::Restore {
        connected.restore(load_view());
    }
    connected.follow_settings();
    let link = Rc::new(RefCell::new(connected));
    wake_for_rechecks();
    let frame = crate::frame_memory::Frame::load();
    let options = WindowOptions {
        title: "Pandemonium".to_owned(),
        size: frame.size,
        min_size: crate::frame_memory::MIN_SIZE,
        decorations: false,
        transparent: true,
        shadow: Some(pm_components::window_shadow()),
        app_id: Some("pandemonium".to_owned()),
        ..WindowOptions::default()
    };
    rustle::run_with(options, move || root(&link, frame));
}

/// The root composable: pumps the link when the server wakes the loop and
/// draws the workspace it holds, maximising the window first when `frame`
/// says it was left maximised.
fn root(link: &Rc<RefCell<Link>>, frame: crate::frame_memory::Frame) {
    let revision = remember(|| 0_u64);
    let installed = remember(|| false);
    if !installed.get() {
        installed.set(true);
        if frame.maximised {
            current_window().toggle_maximize();
        }
        install_close_handler(link, &revision);
        let link = link.clone();
        let revision = revision.clone();
        set_external_handler(move || {
            let mut link = link.borrow_mut();
            if link.pump() {
                revision.update(|value| *value += 1);
            }
            answered_outcomes(&mut link);
            for title in link.take_finished() {
                agent_finished(&link.settings().window.agents, &title);
            }
            if let Some(staged) = link.take_staged() {
                install_update(staged);
            }
            if let Some(reason) = take_install_failure() {
                link.update_failed(reason);
                revision.update(|value| *value += 1);
            }
        });
        rustle::external_waker()();
    }
    let _ = revision.get();
    let settings_open = remember(|| false);
    let actions = actions(link, &revision, &settings_open);
    let workspace = link.borrow().workspace().clone();
    let theme = window_theme(link);
    let bindings = key_bindings(link, &revision, &settings_open);
    framed_kept(true, || {
        provide_key_bindings(bindings, || workspace_page(&workspace, theme, &actions))
    });
    if settings_open.get() {
        settings_window(link, &revision, &settings_open);
    }
    if workspace.picker.is_some() {
        picker_window(link, &revision);
    }
}

/// Has the main window, when it is about to close by its own button, a key or the desktop, ask first while agents work and the settings say so, and otherwise shut the browser down and write the layout before it goes.
fn install_close_handler(link: &Rc<RefCell<Link>>, revision: &State<u64>) {
    let link = link.clone();
    let revision = revision.clone();
    rustle::set_close_handler(move || {
        let Ok(mut held) = link.try_borrow_mut() else {
            return false;
        };
        if !held.may_close() {
            drop(held);
            revision.update(|value| *value += 1);
            return false;
        }
        held.shutdown_browser();
        let view = held.view().write();
        if let Err(error) = crate::store_writer::save_now(VIEW, &view) {
            eprintln!("pandemonium: could not save the layout: {error}");
        }
        true
    });
}

/// The keymap the settings make, handed to the controls holding the keyboard, built again only when the bindings changed; the actions that work everywhere it hands back are done on `link` like the window's own keys.
fn key_bindings(
    link: &Rc<RefCell<Link>>,
    revision: &State<u64>,
    settings_open: &State<bool>,
) -> KeyBindings {
    let keymap = link.borrow().settings().editor.keymap();
    let kept = remember(|| None::<KeyBindings>);
    if let Some(bindings) = kept.get().filter(|bindings| *bindings.keymap() == keymap) {
        return bindings;
    }
    let link = link.clone();
    let revision = revision.clone();
    let settings_open = settings_open.clone();
    let handle = current_window();
    let bindings = KeyBindings::new(keymap, move |action| {
        let Ok(mut held) = link.try_borrow_mut() else {
            return;
        };
        if let Some(outcome) = held.run_action(action) {
            take_outcome(&mut held, outcome, &settings_open, handle);
            save_after(&mut held);
        }
        drop(held);
        revision.update(|value| *value += 1);
    });
    kept.set(Some(bindings.clone()));
    bindings
}

/// Draws `content` inside the self-drawn frame every window shares: the
/// shadow band around it, where the compositor needs one, and the resize
/// grips along its edges.
fn framed(content: impl FnOnce()) {
    framed_kept(false, content);
}

/// Draws `content` inside the self-drawn frame, as [`framed`] does, keeping
/// the window's size and whether it is maximised for the next run when
/// `kept`; the resize grips sit on the frame's own edge and only while it floats.
fn framed_kept(kept: bool, content: impl FnOnce()) {
    let band = remember(|| 0.0_f32);
    let floating = remember(|| true);
    let floated = remember(|| crate::frame_memory::Frame::load().size);
    let whole = remember(rustle::Size::default);
    let sized_to = whole.clone();
    let measured = band.clone();
    let shape = floating.clone();
    let handle = current_window();
    let sized = Modifier::new()
        .fill_max_size()
        .on_size_changed(move |size| {
            if sized_to.get() != size {
                sized_to.set(size);
            }
            let now = handle.shadow_band(pm_components::SHADOW_MARGIN);
            if now != measured.get() {
                measured.set(now);
            }
            let maximised = handle.is_maximized();
            let floats = !maximised && !handle.is_fullscreen();
            if floats != shape.get() {
                shape.set(floats);
            }
            if kept && !handle.is_fullscreen() {
                if floats {
                    floated.set((size.width, size.height));
                }
                crate::frame_memory::Frame {
                    size: floated.get(),
                    maximised,
                }
                .save();
            }
        });
    let inset = if floating.get() { band.get() } else { 0.0 };
    let size = whole.get();
    let area = rustle::Rect::new(
        rustle::Offset::new(inset, inset),
        rustle::Size::new(
            (size.width - inset * 2.0).max(0.0),
            (size.height - inset * 2.0).max(0.0),
        ),
    );
    stack_with(sized, || {
        pm_components::shadow_band(band.get(), floating.get(), || {
            if size.width > 0.0 {
                pm_components::provide_window_area(area, content);
            } else {
                content();
            }
        });
        if floating.get() {
            let inset = (band.get() - GRIP).max(0.0);
            stack_with(Modifier::new().fill_max_size().padding(inset), resize_grips);
        }
    });
}

/// The options of an undecorated window titled `title`, `size` big and at
/// least `min` big, with the shadow and transparency the frame needs.
fn framed_options(title: &str, size: (f32, f32), min: (f32, f32)) -> WindowOptions {
    WindowOptions {
        shadow: Some(pm_components::window_shadow()),
        ..WindowOptions::new(title)
            .with_size(size.0, size.1)
            .with_min_size(min.0, min.1)
            .with_decorations(false)
            .with_transparent(true)
            .with_app_id("pandemonium")
    }
}

/// The folder picker's window, open while the link holds a picker, adding the folder chosen as a project.
fn picker_window(link: &Rc<RefCell<Link>>, revision: &State<u64>) {
    let Some(picker) = link.borrow().workspace().picker.clone() else {
        return;
    };
    let act: Rc<dyn Fn(ProjectAction)> = {
        let link = link.clone();
        let revision = revision.clone();
        Rc::new(move |action: ProjectAction| {
            link.borrow_mut().project_action(action);
            revision.update(|value| *value += 1);
        })
    };
    let options = framed_options("Choose a folder", PICKER_SIZE, (480.0, 320.0));
    let theme = window_theme(link);
    let close = act.clone();
    window(
        options,
        move || close(ProjectAction::Picker(PickerAction::Close)),
        move || {
            let handle = current_window();
            let actions = PickerActions {
                act,
                drag: Rc::new(move || handle.drag_window()),
            };
            framed(|| folder_picker_page(&picker, theme, &actions));
        },
    );
}

/// The settings window, open while `open` holds, changing the settings `link` keeps and saving those the window keeps itself.
fn settings_window(link: &Rc<RefCell<Link>>, revision: &State<u64>, open: &State<bool>) {
    let model = link.borrow().settings().clone();
    let close = {
        let open = open.clone();
        move || open.set(false)
    };
    let options = framed_options("Settings", SETTINGS_SIZE, (640.0, 420.0));
    let theme = window_theme(link);
    let link = link.clone();
    let revision = revision.clone();
    let closing = close.clone();
    window(options, close, move || {
        let handle = current_window();
        let actions = SettingsActions {
            change: Rc::new(move |change: SettingsChange| {
                let mut link = link.borrow_mut();
                link.change_settings(change);
                save_settings(link.settings());
                revision.update(|value| *value += 1);
            }),
            close: Rc::new(closing.clone()),
            drag: Rc::new(move || handle.drag_window()),
            open_url: Rc::new(|url: String| {
                rustle::open_url(&url);
            }),
        };
        framed(|| settings_page(&model, theme, &actions));
    });
}

/// The Pandemonium theme the appearance settings `link` keeps choose, following the desktop's light or dark while the theme mode says so; the file icons learn what the languages and icon themes installed give on the way.
fn window_theme(link: &Rc<RefCell<Link>>) -> AppTheme {
    let link = link.borrow();
    pm_pages::learn_file_icons(link.settings());
    pm_pages::window_theme(link.settings(), rustle::is_system_in_dark_theme())
}

/// The file the editor's settings are kept in.
const EDITOR_SETTINGS: &str = "editor-settings.json";

/// The editor's settings as they were last saved, or as they come.
fn load_editor_settings() -> pm_editor::EditorSettings {
    pm_store::load_text(EDITOR_SETTINGS)
        .map(|text| pm_editor::EditorSettings::read(&text))
        .unwrap_or_default()
}

/// The window's own settings as they were last saved, or as they come.
fn load_window_settings() -> pm_config::WindowSettings {
    pm_store::load_text(WINDOW_SETTINGS)
        .map(|text| pm_config::WindowSettings::read(&text))
        .unwrap_or_default()
}

/// Has the editor's and the window's own settings in `model` written to their files, off the UI thread, when they changed.
fn save_settings(model: &pm_shell::SettingsModel) {
    crate::store_writer::save(EDITOR_SETTINGS, model.editor.write());
    crate::store_writer::save(WINDOW_SETTINGS, model.window.write());
}

/// The installed font families, and those among them made for code, judged by their names.
fn font_families() -> (Vec<String>, Vec<String>) {
    let all = rustle::font_families();
    let mono = all
        .iter()
        .filter(|name| {
            let name = name.to_lowercase();
            [
                "mono", "code", "courier", "consol", "menlo", "fixed", "terminal",
            ]
            .iter()
            .any(|word| name.contains(word))
        })
        .cloned()
        .collect();
    (all, mono)
}

/// The file the window keeps its layout in between runs.
const VIEW: &str = "view.json";

/// The layout the window was left in, or the first one.
fn load_view() -> pm_shell::View {
    pm_store::load_text(VIEW)
        .map(|text| pm_shell::View::read(&text))
        .unwrap_or_default()
}

/// Has the layout `link` keeps written to its file, off the UI thread, when it changed.
fn save_view(link: &Link) {
    crate::store_writer::save(VIEW, link.view().write());
}

/// Does what `link` asked of the window when answers came in, such as putting a link to a line on the clipboard or opening it.
fn answered_outcomes(link: &mut Link) {
    for outcome in link.take_outcomes() {
        match outcome {
            QuickOutcome::Copy(text) => {
                if let Some(clipboard) = rustle::clipboard() {
                    clipboard.set_text(&text);
                }
            }
            QuickOutcome::OpenAddress(url) => {
                rustle::open_url(&url);
            }
            _ => {}
        }
    }
}

/// Saves what changed after an action on `link`: the layout, and the settings when the link changed them itself, after doing what it asked of the window meanwhile.
fn save_after(link: &mut Link) {
    answered_outcomes(link);
    if link.take_settings_changed() {
        save_settings(link.settings());
    }
    save_view(link);
}

/// The file the window keeps its settings in.
const WINDOW_SETTINGS: &str = "window-settings.json";

/// Tells the reader the agent of the session titled `title` finished its
/// turn, the way the agent settings `settings` say.
fn agent_finished(settings: &pm_config::AgentSettings, title: &str) {
    if let Some(notice) = pm_shell::finish_notice(settings, title) {
        crate::notify::tell(notice.how, &notice.title, &notice.body);
    }
}

/// Wakes the window's loop now and then from a thread of its own, so the link asks for the newest release again after [`pm_shell::RELEASE_RECHECK`] even while nothing else happens.
fn wake_for_rechecks() {
    let wake = rustle::external_waker();
    std::thread::Builder::new()
        .name("release-recheck".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(pm_shell::RELEASE_RECHECK / 6);
                wake();
            }
        })
        .ok();
}

thread_local! {
    /// Why installing the downloaded release failed, once the thread doing it said so.
    static INSTALL_FAILURE: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
}

/// Takes why installing the downloaded release failed, when the installing thread said so since the last call.
fn take_install_failure() -> Option<String> {
    INSTALL_FAILURE.with(|held| held.lock().ok().and_then(|mut held| held.take()))
}

/// Installs the release the server downloaded to `staged` over the running one on a thread of its own and restarts into it, ending this process once the new one runs; a failure is left for [`take_install_failure`] and the loop woken, the running install then left as it was.
fn install_update(staged: String) {
    let failure = INSTALL_FAILURE.with(std::sync::Arc::clone);
    let wake = rustle::external_waker();
    std::thread::Builder::new()
        .name("update-install".to_owned())
        .spawn(move || {
            let outcome = pm_update::install(std::path::Path::new(&staged))
                .and_then(|installed| pm_update::relaunch(&installed));
            match outcome {
                Ok(()) => std::process::exit(0),
                Err(reason) => {
                    if let Ok(mut held) = failure.lock() {
                        *held = Some(reason);
                    }
                    wake();
                }
            }
        })
        .ok();
}

/// How thick the invisible resize strips along the window's edges are.
const GRIP: f32 = 4.0;

/// Invisible strips along each edge and corner of the undecorated window that
/// resize it when pressed.
fn resize_grips() {
    let edges = [
        (ResizeEdge::North, BoxAlignment::TopCenter, None, Some(GRIP)),
        (
            ResizeEdge::South,
            BoxAlignment::BottomCenter,
            None,
            Some(GRIP),
        ),
        (
            ResizeEdge::West,
            BoxAlignment::CenterStart,
            Some(GRIP),
            None,
        ),
        (ResizeEdge::East, BoxAlignment::CenterEnd, Some(GRIP), None),
        (
            ResizeEdge::NorthWest,
            BoxAlignment::TopStart,
            Some(GRIP * 2.0),
            Some(GRIP * 2.0),
        ),
        (
            ResizeEdge::NorthEast,
            BoxAlignment::TopEnd,
            Some(GRIP * 2.0),
            Some(GRIP * 2.0),
        ),
        (
            ResizeEdge::SouthWest,
            BoxAlignment::BottomStart,
            Some(GRIP * 2.0),
            Some(GRIP * 2.0),
        ),
        (
            ResizeEdge::SouthEast,
            BoxAlignment::BottomEnd,
            Some(GRIP * 2.0),
            Some(GRIP * 2.0),
        ),
    ];
    let window = current_window();
    for (edge, alignment, width, height) in edges {
        let modifier = Modifier::new().align(alignment);
        let modifier = match width {
            Some(width) => modifier.width(width),
            None => modifier.fill_max_width(),
        };
        let modifier = match height {
            Some(height) => modifier.height(height),
            None => modifier.fill_max_height(),
        };
        spacer(
            modifier
                .cursor(edge.cursor())
                .on_press(move |_| window.drag_resize_window(edge)),
        );
    }
}

/// How the window changes the link and composes again.
type Change = Rc<dyn Fn(&dyn Fn(&mut Link))>;

/// The actions of the dock: its shells' tabs, the terminal's selection, scrolling and menu, the tasks and debugging, each made from `change`; the fields of [`Actions`] the window fills from them, the rest inert.
fn dock_actions(change: &Change) -> Actions {
    let inert = Actions::inert();
    let make = || change.clone();
    let (new, pick, close, scroll, select, command) =
        (make(), make(), make(), make(), make(), make());
    let (run, refresh, start, debug, breakpoint) = (make(), make(), make(), make(), make());
    Actions {
        terminal_new: Rc::new(move || new(&|link| link.new_terminal())),
        terminal_pick: Rc::new(move |key| pick(&|link| link.pick_terminal(key))),
        terminal_close: Rc::new(move |key| close(&|link| link.close_terminal(key))),
        terminal_scroll: Rc::new(move |lines| scroll(&|link| link.scroll_terminal(lines))),
        terminal_select: Rc::new(move |selection| {
            select(&|link| {
                link.select_terminal(selection.anchor, selection.head, selection.clicks);
                if link.settings().window.terminal.copy_on_select {
                    copy_terminal_selection(link);
                }
            })
        }),
        terminal_command: Rc::new(move |chosen| command(&|link| terminal_command(link, chosen))),
        run_task: Rc::new(move |index| run(&|link| link.run_task(index))),
        refresh_dock: Rc::new(move || refresh(&|link| link.refresh_dock())),
        debug_start: Rc::new(move |index| start(&|link| link.debug_start(index))),
        debug_command: Rc::new(move |chosen| debug(&|link| link.debug_command(chosen.clone()))),
        toggle_breakpoint: Rc::new(move |path, line| {
            breakpoint(&|link| link.toggle_breakpoint(path.clone(), line))
        }),
        ..inert
    }
}

/// Puts the text marked in `link`'s shown shell on the system clipboard, when something is marked.
fn copy_terminal_selection(link: &mut Link) {
    if let (Some(text), Some(clipboard)) = (link.terminal_selection(), rustle::clipboard()) {
        clipboard.set_text(&text);
    }
}

/// Runs `command` of the terminal's menu on `link`'s shown shell, through the system clipboard for copying and pasting.
fn terminal_command(link: &mut Link, command: TerminalCommand) {
    match command {
        TerminalCommand::Copy => copy_terminal_selection(link),
        TerminalCommand::Paste => {
            if let Some(text) = rustle::clipboard().and_then(|clipboard| clipboard.text()) {
                link.paste_terminal(&text);
            }
        }
        TerminalCommand::SelectAll => link.select_all_terminal(),
        TerminalCommand::Clear => link.clear_terminal(),
    }
}

/// The actions the pages call, each changing the link and bumping `revision`
/// so the window composes again.
fn actions(
    link: &Rc<RefCell<Link>>,
    revision: &State<u64>,
    settings_open: &State<bool>,
) -> Actions {
    let change = {
        let link = link.clone();
        let revision = revision.clone();
        let change: Change = Rc::new(move |change: &dyn Fn(&mut Link)| {
            change(&mut link.borrow_mut());
            save_after(&mut link.borrow_mut());
            revision.update(|value| *value += 1);
        });
        change
    };
    let resize = change.clone();
    let update = change.clone();
    let sign_in = change.clone();
    let open = change.clone();
    let send = change.clone();
    let right = change.clone();
    let dock = change.clone();
    let toggle = change.clone();
    let typed = change.clone();
    let new_session = change.clone();
    let choose = change.clone();
    let start = change.clone();
    let navigate = change.clone();
    let trail = change.clone();
    let overview = change.clone();
    let reopen = change.clone();
    let extension = change.clone();
    let git = change.clone();
    let open_file = change.clone();
    let file_tab = change.clone();
    let save_file = change.clone();
    let file_action = change.clone();
    let dismiss_popups = change.clone();
    let code = {
        let link = link.clone();
        let change = change.clone();
        move |index: usize, ask: pm_editor::ServerAsk| {
            if matches!(ask, pm_editor::ServerAsk::Changed(..)) {
                if let Ok(mut held) = link.try_borrow_mut() {
                    held.code(index, ask);
                }
            } else {
                change(&|held| held.code(index, ask.clone()));
            }
        }
    };
    let centre = change.clone();
    let located = change.clone();
    let tree = change.clone();
    let search = change.clone();
    let projects = change.clone();
    let adding = change.clone();
    let composed = change.clone();
    let quick = quick_action(link, revision, settings_open);
    let shortcut = shortcut(link, revision, settings_open);
    let dock_actions = dock_actions(&change);
    let answer = change.clone();
    let inquire = change.clone();
    let trusted = change.clone();
    let pull = change.clone();
    let command = change.clone();
    let carry = change.clone();
    let address = change.clone();
    let browser = browser_action(link, revision, &change);
    let closing = link.clone();
    let settings_open_kept = settings_open.clone();
    let settings_open = settings_open.clone();
    let resized = {
        let link = link.clone();
        let revision = revision.clone();
        move |columns, rows| {
            if link.borrow_mut().resize_terminal(columns, rows) {
                revision.update(|value| *value += 1);
            }
        }
    };
    let handle = current_window();
    let bump = revision.clone();
    let session = session_action(link, revision, settings_open_kept.clone());
    Actions {
        open_session: Rc::new(move |at| open(&|link| link.open_session(at.clone()))),
        new_session: Rc::new(move |at| new_session(&|link| link.new_session(at.clone()))),
        send_prompt: Rc::new(move |text| send(&|link| link.send_prompt(text.clone()))),
        composer: Rc::new(move |action| match action {
            pm_shell::ComposerAction::Browse => {
                let paths = crate::file_dialog::pick_attachments();
                if !paths.is_empty() {
                    composed(&|link| {
                        link.composer_action(pm_shell::ComposerAction::Attach(paths.clone()))
                    });
                }
            }
            action => composed(&|link| link.composer_action(action.clone())),
        }),
        choose_agent: Rc::new(move |index| choose(&|link| link.choose_agent(index))),
        start: Rc::new(move |text| start(&|link| link.start(text.clone()))),
        right_tab: Rc::new(move |tab| right(&|link| link.choose_right_tab(tab))),
        dock_tab: Rc::new(move |tab| dock(&|link| link.workspace_mut().dock_tab = tab)),
        terminal_input: Rc::new(move |bytes| typed(&|link| link.terminal_input(bytes.clone()))),
        terminal_resize: Rc::new(resized),
        terminal_new: dock_actions.terminal_new,
        terminal_pick: dock_actions.terminal_pick,
        terminal_close: dock_actions.terminal_close,
        terminal_scroll: dock_actions.terminal_scroll,
        terminal_select: dock_actions.terminal_select,
        terminal_command: dock_actions.terminal_command,
        run_task: dock_actions.run_task,
        refresh_dock: dock_actions.refresh_dock,
        debug_start: dock_actions.debug_start,
        debug_command: dock_actions.debug_command,
        toggle_breakpoint: dock_actions.toggle_breakpoint,
        toggle: Rc::new(move |panel| {
            toggle(&|link| {
                let workspace = link.workspace_mut();
                match panel {
                    Panel::Dock => workspace.dock_open = !workspace.dock_open,
                    Panel::Right => workspace.right_open = !workspace.right_open,
                    Panel::Sidebar => workspace.sidebar_open = !workspace.sidebar_open,
                }
            })
        }),
        command_centre: Rc::new(move || {
            centre(&|link| {
                let _ = link.quick(QuickAction::Open(String::new()));
            })
        }),
        navigate: Rc::new(move |destination| match destination {
            Destination::NewChat => navigate(&|link| link.new_chat()),
            Destination::History => navigate(&|link| link.navigate(Page::History)),
            Destination::Extensions => navigate(&|link| link.navigate(Page::Extensions)),
            Destination::Settings => {
                navigate(&|link| {
                    let (all, mono) = font_families();
                    let model = link.settings_mut();
                    model.fonts = all;
                    model.mono_fonts = mono;
                    link.open_settings();
                });
                settings_open.set(true);
            }
            Destination::NewWorkspace => {
                adding(&|link| link.project_action(ProjectAction::Adding(Some(AddStep::Choose))))
            }
        }),
        window: Rc::new(move |action| match action {
            WindowAction::Minimise => handle.minimize(),
            WindowAction::Maximise => handle.toggle_maximize(),
            WindowAction::Close => handle.close(),
            WindowAction::ConfirmClose => {
                closing.borrow_mut().confirm_close(true);
                handle.close();
            }
            WindowAction::CancelClose => {
                closing.borrow_mut().confirm_close(false);
                bump.update(|value| *value += 1);
            }
            WindowAction::Drag => handle.drag_window(),
            WindowAction::Back => trail(&|link| link.back()),
            WindowAction::Forward => trail(&|link| link.forward()),
        }),
        open_overview: Rc::new(move |path| {
            overview(&|link| link.navigate(Page::Overview(path.clone())))
        }),
        reopen_session: Rc::new(move |id| reopen(&|link| link.reopen_session(id))),
        press_extension: Rc::new(move |id| extension(&|link| link.press_extension(&id))),
        choose: Rc::new(move |ticket, option| answer(&|link| link.choose(ticket, option.clone()))),
        answer_inquiry: Rc::new(move |ticket, replies| {
            inquire(&|link| link.answer_inquiry(ticket, replies.clone()))
        }),
        trust: Rc::new(move |ticket, verdict| trusted(&|link| link.trust(ticket, verdict))),
        pull_request: Rc::new(move |action| pull(&|link| link.pull_request_action(action.clone()))),
        conversation: Rc::new(move |order| {
            command(&|link| link.conversation_command(order.clone()))
        }),
        continue_chat: Rc::new(move |at| carry(&|link| link.continue_chat(at.clone()))),
        open_address: Rc::new(move |url| address(&|link| link.open_address(&url))),
        browser,
        update: Rc::new(move |action| match action {
            UpdateAction::Open => update(&|link| link.show_update(true)),
            UpdateAction::Close => update(&|link| link.show_update(false)),
            UpdateAction::Install => update(&|link| match pm_update::blocker() {
                Some(reason) => link.update_failed(reason),
                None => link.start_update(),
            }),
            UpdateAction::ViewPage => update(&|link| {
                if let Some(offer) = &link.workspace().update {
                    rustle::open_url(&offer.url);
                }
            }),
        }),
        sign_in: Rc::new(move |action| {
            sign_in(&|link| {
                let Some(code) = link.workspace().sign_in.clone() else {
                    return;
                };
                match action {
                    SignInAction::Hide => link.hide_sign_in(),
                    SignInAction::Copy => {
                        if let Some(clipboard) = rustle::clipboard() {
                            clipboard.set_text(&code.code);
                        }
                    }
                    SignInAction::Open => {
                        rustle::open_url(&code.url);
                    }
                }
            })
        }),
        resize: Rc::new(move |panel, size| {
            resize(&|link| {
                let sizes = &mut link.workspace_mut().sizes;
                match panel {
                    Panel::Sidebar => sizes.sidebar = size,
                    Panel::Right => sizes.right = size,
                    Panel::Dock => sizes.dock = size,
                }
            })
        }),
        git: Rc::new(move |action| git(&|link| link.git(action.clone()))),
        open_file: Rc::new(move |path| open_file(&|link| link.open_file(path.clone()))),
        file_tab: Rc::new(move |action| file_tab(&|link| link.file_tab(action))),
        save_file: Rc::new(move |index| save_file(&|link| link.save_file(index))),
        code: Rc::new(code),
        file_action: Rc::new(move |index, action| {
            file_action(&|link| link.file_action(index, action.clone()))
        }),
        dismiss_popups: Rc::new(move |index| dismiss_popups(&|link| link.dismiss_popups(index))),
        open_location: Rc::new(move |path, line| {
            located(&|link| link.open_location(path.clone(), line))
        }),
        tree: Rc::new(move |action| tree(&|link| link.tree(action.clone()))),
        search: Rc::new(move |action| search(&|link| link.search(action.clone()))),
        quick,
        shortcut,
        projects: Rc::new(move |action| projects(&|link| link.project_action(action.clone()))),
        session,
    }
}

/// What the sidebar's session menu, the session dialogs and the status bar ask: done on `link`, then whatever it asks of the window beyond the link, such as copying, and the layout saved and `revision` bumped.
fn session_action(
    link: &Rc<RefCell<Link>>,
    revision: &State<u64>,
    settings_open: State<bool>,
) -> Rc<dyn Fn(SessionCommand)> {
    let link = link.clone();
    let revision = revision.clone();
    let handle = current_window();
    Rc::new(move |command| {
        let mut held = link.borrow_mut();
        let outcome = held.session_command(command);
        take_outcome(&mut held, outcome, &settings_open, handle);
        save_after(&mut held);
        drop(held);
        revision.update(|value| *value += 1);
    })
}

/// The browser pane's action: the pointer, keys, sizes and focus handed to a page change the link without saving the layout, composing again only when a picture or a tab changed; everything else goes through `change`.
fn browser_action(
    link: &Rc<RefCell<Link>>,
    revision: &State<u64>,
    change: &Change,
) -> Rc<dyn Fn(BrowserAction)> {
    let link = link.clone();
    let revision = revision.clone();
    let change = change.clone();
    Rc::new(move |action| {
        let quiet = matches!(
            action,
            BrowserAction::Input(..) | BrowserAction::Resized(..) | BrowserAction::Focused(..)
        );
        if !quiet {
            change(&|link| {
                link.browser(action.clone());
            });
        } else if link.borrow_mut().browser(action) {
            revision.update(|value| *value += 1);
        }
    })
}

/// The quick input's action: changes the link, then opens the file, opens the
/// settings window or saves the settings when the row picked asks for it.
fn quick_action(
    link: &Rc<RefCell<Link>>,
    revision: &State<u64>,
    settings_open: &State<bool>,
) -> Rc<dyn Fn(QuickAction)> {
    let link = link.clone();
    let revision = revision.clone();
    let settings_open = settings_open.clone();
    let handle = current_window();
    Rc::new(move |action| {
        let mut held = link.borrow_mut();
        let outcome = held.quick(action);
        take_outcome(&mut held, outcome, &settings_open, handle);
        save_after(&mut held);
        drop(held);
        revision.update(|value| *value += 1);
    })
}

/// Does what `outcome` of a quick input row, a key or a menu asks of the window `handle` beyond `link`: opens a file, opens the settings window through `settings_open`, saves the settings, copies text, shows a folder, switches the appearance or closes the window.
fn take_outcome(
    link: &mut Link,
    outcome: Option<QuickOutcome>,
    settings_open: &State<bool>,
    handle: WindowHandle,
) {
    match outcome {
        Some(QuickOutcome::OpenFile(path, None)) => link.open_file(path),
        Some(QuickOutcome::OpenFile(path, Some(line))) => link.open_location(path, line),
        Some(QuickOutcome::Settings) => {
            let (all, mono) = font_families();
            let model = link.settings_mut();
            model.fonts = all;
            model.mono_fonts = mono;
            link.open_settings();
            settings_open.set(true);
        }
        Some(QuickOutcome::SettingsChanged) => save_settings(link.settings()),
        Some(QuickOutcome::Copy(text)) => {
            if let Some(clipboard) = rustle::clipboard() {
                clipboard.set_text(&text);
            }
        }
        Some(QuickOutcome::OpenFolder(folder)) => {
            rustle::open_url(&format!("file://{folder}"));
        }
        Some(QuickOutcome::ToggleTheme) => {
            let dark = window_theme_is_dark(link);
            link.settings_mut().window.appearance.theme_mode = if dark {
                pm_config::ThemeMode::Light
            } else {
                pm_config::ThemeMode::Dark
            };
            save_settings(link.settings());
        }
        Some(QuickOutcome::Quit) => handle.close(),
        Some(QuickOutcome::OpenAddress(url)) => {
            rustle::open_url(&url);
        }
        None => {}
    }
}

/// Whether the appearance the settings of `link` choose is dark now, following the desktop while they say so.
fn window_theme_is_dark(link: &Link) -> bool {
    match link.settings().window.appearance.theme_mode {
        pm_config::ThemeMode::Dark => true,
        pm_config::ThemeMode::Light => false,
        pm_config::ThemeMode::System => rustle::is_system_in_dark_theme(),
    }
}

/// The window's keys: hands each chord to the keymap through `link`, and when it was taken does what it asks, saves the layout and bumps `revision`.
fn shortcut(
    link: &Rc<RefCell<Link>>,
    revision: &State<u64>,
    settings_open: &State<bool>,
) -> Rc<dyn Fn(String) -> bool> {
    let link = link.clone();
    let revision = revision.clone();
    let settings_open = settings_open.clone();
    let handle = current_window();
    Rc::new(move |chord| {
        let mut held = link.borrow_mut();
        let pressed = held.press_key(&chord);
        if !pressed.taken {
            return false;
        }
        take_outcome(&mut held, pressed.outcome, &settings_open, handle);
        save_after(&mut held);
        drop(held);
        revision.update(|value| *value += 1);
        true
    })
}
