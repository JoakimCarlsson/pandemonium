//! The window's shell without drawing: the models the pages show, the
//! sample workspace the snapshots render, and the link to the server that
//! fills the models with real projects and sessions, and the settings
//! window's model: every setting, how each stands and changes, and the
//! sections they are shown in.

mod browser;
mod browser_action;
mod browser_address;
mod browser_links;
mod browser_safety;
mod browser_sample;
mod browser_scope;
mod browser_tools;
mod composer;
mod conversation;
mod dialog;
mod dock;
mod dock_model;
mod dock_sample;
mod extensions;
mod file_actions;
mod file_tree;
mod finish;
mod git;
mod git_sample;
mod grammar;
mod history;
mod link;
mod model;
mod open_files;
mod page;
mod projects;
mod quick;
mod review;
mod review_sample;
mod sample;
mod sample_code;
mod sample_composer;
mod sample_conversation;
mod sample_files;
mod search;
mod settings_catalog;
mod settings_change;
mod settings_forms;
mod settings_keymap;
mod settings_model;
mod settings_sample;
mod settings_section;
mod settings_values;
mod start;
mod terminal;
mod thread;
mod translate;
mod view;
mod web;

pub use browser::{
    Browser, BrowserMenu, BrowserTab, Command as BrowserCommand, HOME, MOST_TABS, PageSearch,
    Picture, SavedTab, TOOLS_LEAST, TOOLS_WIDTH, ZoomStep, next_tab_number,
};
pub use browser_action::{BrowserAction, PageCommand, Surface};
pub use browser_address::{SEARCH_URL, address_of};
pub use browser_links::{Route, local_address, route};
pub use browser_safety::{ALLOW_DELAY, Answer, Certificate, Download, Question, Safety, Saving};
pub use browser_sample::{BrowserSample, sample_browser};
pub use browser_scope::{BrowserShelf, NO_SCOPE, SavedBrowser};
pub use browser_tools::{agent_may_open, changes_page, locked_message, origin_allowed, tabs_text};
pub use composer::{
    ACCOUNT_CHOICE, ADD_ACCOUNT, OWN_ACCOUNT, Capture, CaptureKind, ComposerAction, ComposerMenu, ComposerState, ConversationSetting, Gauge,
    GoalChip, PickedElement, PromptFile, QueuedPrompt, SettingPick, SlashCommand, allowance_gauge,
    context_gauge, goal_chip, matching_commands, prompt_text, slot_resolves_to, with_slot_picks,
};
pub use conversation::{
    Activity, ActivityKind, Approval, ApprovalChoice, Block, Extras, InquiryForm, InquiryQuestion,
    InquiryReply, Markdown, Message, Notice, NoticeKind, PlanItem, PlanProgress, PullRequestInfo,
    Speaker, SpendSummary, StepState, ToolKind, ToolStep, TrustAsk, TrustVerdict,
};
pub use dialog::{SignInCode, UpdateOffer, UpdateRun};
pub use dock_model::{
    DebugBreakpoint, DebugCommand, DebugFrameEntry, DebugModel, DebugScenarioEntry,
    DebugScopeEntry, DebugSession, DebugStanding, DebugVariableEntry, InlineValue, Problem,
    ProblemSeverity, TaskEntry, TaskList, TerminalTab,
};
pub use extensions::{
    Capability, Extension, ExtensionAction, ExtensionDo, ExtensionPreview, Tone, language_offer,
    listed,
};
pub use file_actions::FileAction;
pub use file_tree::{
    EditKind, FOLDER_MARK, FileTree, Held, PickHow, TreeAction, TreeEdit, TreeRow, join, name_of,
    name_problem, parent_of,
};
pub use finish::{FinishNotice, finish_notice};
pub use git::{
    BranchMenu, BranchTask, ChangeGroup, ChangedFile, GitAction, GitMenu, GitPrompt, GitSection,
    GitSummary, GraphCommit, GraphEdge, Pick, Primary, SplitRow, side_by_side, tally,
};
pub use git_sample::sample_git;
pub use grammar::{installed_grammar, learn_grammar};
pub use history::{PastSession, Span};
pub use link::{
    BASE_SETTING, ConversationCommand, KeyOutcome, Link, SessionCommand, age, session_at,
};
pub use model::{
    AgentChoice, Commit, ContinueChoice, DockTab, PanelSizes, Project, ProjectReach, Renaming, RightTab, Session,
    SessionStatus, Start, Workspace,
};
pub use open_files::{
    Comparison, EditorBar, EditorPopup, FileBody, OpenFile, OpenFiles, PANE_SHARE_MIN, Pane,
    SavedFile, SessionTab, SharedDocument, TabAction, TabMenu, Unsaved, is_picture,
};
pub use page::{Page, Trail};
pub use pm_browser::CaptureKind as PageCapture;
pub use projects::{
    AddStep, Adder, FolderPicker, GithubRepository, Machine, PickerAction, ProjectAction, Reach, clone_address,
    clone_name, join_host, moved, reordered,
};
pub use quick::{
    COMMANDS, Choosing, Command, QUICK_ROWS, QuickAction, QuickInput, QuickItem, QuickKind,
    QuickMode, QuickOutcome, QuickPick, fuzzy, quick_items,
};
pub use review::{
    ConflictRegion, ConflictSide, ConflictView, LineNote, Revision, conflict_regions,
    grammar_by_ending, language_by_ending, notes_prompt, settle_all, settle_conflict,
};
pub use review_sample::{sample_review, sample_search_edit};
pub use sample::{
    sample_picker, sample_projects, sample_remote_projects, sample_sign_in, sample_update,
    sample_workspace,
};
pub use sample_code::{COLOUR_SAMPLE, CodeSample, RIGHT_TO_LEFT_SAMPLE, sample_code};
pub use sample_composer::{SampleComposer, sample_composer, sample_mentions};
pub use sample_conversation::{SampleAsk, sample_ask};
pub use sample_files::{sample_changes, sample_files, sample_quick, sample_search};
pub use search::{
    EXCERPT_CONTEXT, ProjectSearch, SearchAction, SearchExcerpt, SearchHit, excerpt_ranges,
    splice_excerpts,
};
pub use settings_catalog::{Keeper, LISTS, Setting, found, on_page};
pub use settings_change::{SettingsChange, binding_reads, origin_reads, release_request};
pub use settings_forms::{
    Draft, FormChange, McpField, McpForm, ModelField, ModelForm, SettingsForms,
};
pub use settings_keymap::{KeymapGroup, KeymapRow};
pub use settings_model::{
    AgentSetting, Item, Kept, Kind, REACHES, RUNNING, Release, SHIPPED_THEMES, SettingsModel,
    SettingsPage, ToolSet, reach_of, slot_source,
};
pub use settings_sample::{sample_forms, sample_settings};
pub use settings_section::Section;
pub use settings_values::Control;
pub use start::{offered_agents, show_start, start_chat, start_in, title_of};
pub use terminal::{TerminalScreen, WindowSpan};
pub use view::{SavedPage, View};
pub use web::{Wake, Web};
