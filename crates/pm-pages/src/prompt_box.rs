//! The open session's prompt box: the prompts queued behind the turn, and
//! the composer with what is attached, the agent's settings and commands,
//! the context and allowance rings, and the send and stop buttons.

use pm_components::{
    Composer, ComposerChoice, ComposerMenu, ComposerPick, QueuedList, UsageRing, UsageRingVariant,
};
use pm_shell::{
    ACCOUNT_CHOICE, ADD_ACCOUNT, CaptureKind, ComposerAction, ComposerState, ConversationSetting,
    OWN_ACCOUNT, Workspace,
};
use rustle::prelude::*;

use crate::actions::{Actions, Destination};

/// The category of the setting naming the model.
const MODEL: &str = "model";

/// The category of the setting naming the mode, such as the permissions.
const MODE: &str = "mode";

/// Emits the prompts queued behind the open session's turn and its composer, `width` wide.
pub fn prompt_box(workspace: &Workspace, actions: &Actions, width: f32) {
    let composer = &workspace.extras.composer;
    let act = actions.composer.clone();
    let mut queued = QueuedList::new().modifier(Modifier::new().width(width));
    for prompt in &composer.queued {
        queued = queued.item(prompt.id, prompt.text.clone(), prompt.files.clone());
    }
    let (hurry, remove, hold, edit) = (act.clone(), act.clone(), act.clone(), act.clone());
    queued
        .on_hurry(move |id| hurry(ComposerAction::Hurry(id)))
        .on_remove(move |id| remove(ComposerAction::RemoveQueued(id)))
        .on_hold(move |held| hold(ComposerAction::HoldQueue(held)))
        .on_edit(move |id, text| edit(ComposerAction::EditQueued(id, text)))
        .show();
    let send = actions.send_prompt.clone();
    let mut prompt = Composer::new("Reply to the agent, or type / for commands")
        .prefill(workspace.extras.prefill.clone())
        .prefill_request(workspace.extras.prefills)
        .working(composer.working)
        .menu(composer.menu.as_ref().map(|menu| match menu {
            pm_shell::ComposerMenu::Setting(id) => ComposerMenu::Setting(id.clone()),
            pm_shell::ComposerMenu::Field(across, down) => ComposerMenu::Field(*across, *down),
        }))
        .menu_request(composer.asked)
        .focus_request(composer.asked)
        .modifier(Modifier::new().width(width));
    prompt = with_settings(prompt, workspace);
    for command in &composer.commands {
        prompt = prompt.command(command.name.clone(), command.description.clone());
    }
    prompt = with_attachments(prompt, composer, actions);
    if let Some(context) = &composer.context {
        prompt = prompt.ring(UsageRing::new(context.fraction).detail(context.detail.clone()));
    }
    if let Some(allowance) = &composer.allowance {
        prompt = prompt.ring(
            UsageRing::new(allowance.fraction)
                .variant(UsageRingVariant::Allowance)
                .detail(allowance.detail.clone()),
        );
    }
    let (now, stop, mention) = (act.clone(), act.clone(), act.clone());
    let picker = actions.clone();
    with_attaching(prompt, actions)
        .mentions(workspace.tree.paths.clone())
        .on_mention(move || mention(ComposerAction::Mention))
        .on_send(move |text| send(text))
        .on_send_now(move |text| now(ComposerAction::SendNow(text)))
        .on_stop(move || stop(ComposerAction::Stop))
        .on_pick(move |id, value| pick(id, value, &picker))
        .show();
}

/// `prompt` with a chip for each file, browser element and capture `composer` holds for the next prompt, whose cross takes it off.
pub(crate) fn with_attachments(
    mut prompt: Composer,
    composer: &ComposerState,
    actions: &Actions,
) -> Composer {
    let act = actions.composer.clone();
    for (place, file) in composer.files.iter().enumerate() {
        let detach = act.clone();
        let glyph = if file.picture {
            phosphor::IMAGE
        } else {
            phosphor::PAPERCLIP
        };
        prompt = prompt.attached(glyph, file.name.clone(), move || {
            detach(ComposerAction::DetachFile(place))
        });
    }
    for (place, element) in composer.elements.iter().enumerate() {
        let detach = act.clone();
        prompt = prompt.attached(phosphor::CURSOR_CLICK, element.label.clone(), move || {
            detach(ComposerAction::DetachElement(place))
        });
    }
    for (place, capture) in composer.captures.iter().enumerate() {
        let detach = act.clone();
        let glyph = match capture.kind {
            CaptureKind::Screenshot => phosphor::CAMERA,
            CaptureKind::Console => phosphor::TERMINAL,
        };
        prompt = prompt.attached(glyph, capture.label.clone(), move || {
            detach(ComposerAction::DetachCapture(place))
        });
    }
    prompt
}

/// `prompt` attaching files picked with its `+` button, dropped or pasted on it, and pictures pasted, and cycling the mode with Shift+Tab.
pub(crate) fn with_attaching(prompt: Composer, actions: &Actions) -> Composer {
    let act = actions.composer.clone();
    let (browse, attach, picture, cycle) = (act.clone(), act.clone(), act.clone(), act);
    prompt
        .on_browse(move || browse(ComposerAction::Browse))
        .on_cycle_mode(move || cycle(ComposerAction::CycleMode))
        .on_attach(move |paths| attach(ComposerAction::Attach(paths)))
        .on_picture(move |width, height, rgba| {
            picture(ComposerAction::Picture(width, height, rgba))
        })
}

/// `prompt` with the open session's settings in its toolbar: those the
/// agent lists, each with its menu, or the model and permissions named
/// before the agent said, and the account.
fn with_settings(mut prompt: Composer, workspace: &Workspace) -> Composer {
    let settings = &workspace.extras.composer.settings;
    if settings.is_empty() {
        prompt = prompt
            .choice(None, workspace.model.clone())
            .choice(Some(phosphor::SHIELD_CHECK), workspace.permissions.clone());
    }
    for setting in settings {
        prompt = prompt.setting(choice_of(setting));
    }
    if workspace.accounts.is_empty() {
        return prompt.choice(Some(phosphor::USER_CIRCLE), workspace.account.clone());
    }
    prompt.setting(account_choice(workspace))
}

/// The account chip of the open session: its agent's own login, each named login of its agent and an entry to add one, the one it runs as current.
fn account_choice(workspace: &Workspace) -> ComposerChoice {
    let mut picks = vec![ComposerPick {
        value: OWN_ACCOUNT.to_owned(),
        name: "Default account".to_owned(),
        description: "The agent's own login".to_owned(),
    }];
    picks.extend(
        workspace
            .accounts
            .iter()
            .map(|(id, name)| ComposerPick {
                value: id.clone(),
                name: name.clone(),
                description: String::new(),
            }),
    );
    picks.push(ComposerPick {
        value: ADD_ACCOUNT.to_owned(),
        name: "Add account…".to_owned(),
        description: "Sign in another login in the settings".to_owned(),
    });
    ComposerChoice {
        icon: Some(phosphor::USER_CIRCLE),
        label: workspace.account.clone(),
        id: ACCOUNT_CHOICE.to_owned(),
        heading: "Account".to_owned(),
        current: workspace.login.clone().unwrap_or_default(),
        picks,
    }
}

/// Does what picking `value` of the toolbar setting `id` asks: switching the account, opening the settings to add one, or giving the agent's setting the value.
pub(crate) fn pick(id: String, value: String, actions: &Actions) {
    let act = &actions.composer;
    match (id.as_str(), value.as_str()) {
        (ACCOUNT_CHOICE, ADD_ACCOUNT) => {
            act(ComposerAction::ManageAccounts);
            (actions.navigate)(Destination::Settings);
        }
        (ACCOUNT_CHOICE, OWN_ACCOUNT) => act(ComposerAction::Account(None)),
        (ACCOUNT_CHOICE, login) => act(ComposerAction::Account(Some(login.to_owned()))),
        _ => act(ComposerAction::Set(id, value)),
    }
}

/// The toolbar choice of `setting`, its glyph picked by what it is for.
pub(crate) fn choice_of(setting: &ConversationSetting) -> ComposerChoice {
    let icon = match setting.category.as_str() {
        MODEL => None,
        MODE => Some(phosphor::SHIELD_CHECK),
        "thought_level" => Some(phosphor::BRAIN),
        _ => Some(phosphor::SLIDERS_HORIZONTAL),
    };
    ComposerChoice {
        icon,
        label: setting.current_name().replace(" (recommended)", ""),
        id: setting.id.clone(),
        heading: setting.name.clone(),
        current: setting.current.clone(),
        picks: setting
            .picks
            .iter()
            .map(|pick| ComposerPick {
                value: pick.value.clone(),
                name: pick.name.clone(),
                description: pick.description.clone(),
            })
            .collect(),
    }
}
