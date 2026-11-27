//! The components the app is built from.
//!
//! A component is a composable: a function or a builder that emits what it
//! draws, in place, while the app composes its window. Components are rustle
//! composables and rustle widgets themed through rustle's theme: [`theme`]
//! provides the colour scheme, type scale and shapes rustle's `theme` carries,
//! and one composition local for the app's own tokens (every colour role,
//! syntax colours, font stacks); text is set in rustle `TextStyle`s through the
//! rustle text composables (`rustle::basic_text`), which take part in the
//! selection of a `rustle::selection_container` around them; [`icons`] maps icon names onto
//! rustle's Phosphor set; and what the user does reaches the app through the
//! handler of [`events`], named by focus ids. A builder is made with `new`,
//! given a variant for how it looks and a size for how big it is, and emitted
//! with `build`; the caller puts what goes around it with `.modifier(..)`,
//! replaces its padding with `.inner(..)` and adds children with
//! `build_with(..)`, and never reaches into it. Each component lives in a
//! module named for what it is, which holds its `Variant` and is re-exported
//! here; `Size` is shared.

pub mod activity;
pub mod adder;
pub mod age;
pub mod agent_list;
pub mod annotations;
pub mod areas;
pub mod arrange;
pub mod assist;
pub mod avatar;
pub mod badge;
pub mod bar_row;
pub mod bounds;
pub mod branch_menu;
pub mod branch_tasks;
pub mod browser;
pub mod browser_address;
pub mod browser_chrome;
pub mod browser_clear;
pub mod browser_find;
pub mod browser_keys;
pub mod browser_life;
pub mod browser_links;
pub mod browser_menu;
pub mod browser_pointing;
pub mod browser_privacy;
pub mod browser_profiles;
pub mod browser_reader;
pub mod browser_recipes;
pub mod browser_safety;
pub mod browser_scope;
pub mod browser_steps;
pub mod browser_tab_menu;
pub mod browser_tabs;
pub mod bug_report;
pub mod button;
pub mod captures;
pub mod change_list;
pub mod changes;
pub mod chat;
pub mod chats;
pub mod checkbox;
pub mod choices;
pub mod code_box;
pub mod code_pointing;
pub mod code_view;
pub mod color_picker;
pub mod compare;
pub mod composer;
pub mod conflict;
pub mod context_menu;
pub mod continue_chat;
pub mod conversation_commands;
pub mod debug;
pub mod debug_panel;
pub mod diff;
pub mod diff_syntax;
pub mod dock;
pub mod draft;
pub mod drop_text;
pub mod editor;
pub mod elements;
pub mod events;
pub mod excerpts;
pub mod exhibit;
pub mod extensions;
pub mod fallback;
pub mod figures;
pub mod file_actions;
pub mod file_icon;
pub mod file_tree;
pub mod find;
pub mod focus;
pub mod folder_picker;
pub mod git_controls;
pub mod git_menus;
pub mod git_prompts;
pub mod graph;
mod hand_over;
pub mod hearing;
pub mod held;
pub mod history;
pub mod icons;
pub mod inline_values;
pub mod input;
pub mod inquiry;
pub mod language_offer;
pub mod lazy_diffs;
pub mod lightbulb;
pub mod line_edits;
pub mod listed;
pub mod listing;
pub mod markdown;
pub mod marked;
pub mod marks;
pub mod menu;
pub mod message_menu;
pub mod minimap;
pub mod modal;
pub mod navigate;
pub mod open_files;
pub mod overview;
pub mod pacing;
pub mod panel_sizes;
pub mod panes;
pub mod parts;
pub mod plan;
pub mod pointing;
pub mod popover;
pub mod popups;
pub mod preview;
pub mod problems_panel;
pub mod project;
pub mod project_search;
pub mod pull_request_bar;
pub mod queued;
pub mod quick_choices;
pub mod quick_input;
pub mod quick_sessions;
mod reach;
pub mod review;
mod rows_in_view;
pub mod sample;
pub mod sash;
pub mod scroll_marks;
mod session_attention;
pub mod settings;
pub mod settings_agents;
pub mod settings_catalog;
pub mod settings_drafts;
pub mod settings_editor;
pub mod settings_keymap;
pub mod settings_languages;
pub mod settings_library;
pub mod settings_machines;
pub mod settings_models;
pub mod settings_orchestration;
pub mod settings_pages;
pub mod settings_row;
pub mod settings_rows;
pub mod settings_servers;
pub mod settings_slots;
pub mod settings_state;
pub mod settings_usage;
pub mod settings_values;
pub mod shared_checkout;
pub mod sidebar;
pub mod sign_in_code;
pub mod size;
pub mod slot_picks;
pub mod source_actions;
pub mod spend;
pub mod state;
pub mod stepper;
pub mod steps;
pub mod sticky;
pub mod surface;
pub mod tab_chip;
pub mod tab_menu;
pub mod tab_steps;
pub mod tagged;
pub mod tally;
pub mod target;
pub mod tasks;
pub mod terminal;
pub mod terminal_keys;
pub mod terminal_menu;
pub mod text_lines;
pub mod theme;
pub mod tile;
pub mod title_bar;
pub mod tooltip;
pub mod trail;
pub mod tree;
pub mod tree_actions;
pub mod trust;
pub mod update;
pub mod update_dialog;
pub mod usage;
pub mod viewer;
pub mod vim;
pub mod welcome;
pub mod window;

pub use avatar::Avatar;
pub use badge::Badge;
pub use bar_row::BarRow;
pub use button::Button;
pub use changes::Changes;
pub use checkbox::Checkbox;
pub use code_box::CodeBox;
pub use conflict::Conflict;
pub use context_menu::EditorCommand;
pub use held::Held;
pub use input::Input;
pub use markdown::Markdown;
pub use menu::Menu;
pub use modal::Modal;
pub use popover::Popover;
pub use project::Project;
pub use sash::Sash;
pub use size::Size;
pub use stepper::Stepper;
pub use surface::Surface;
pub use tally::Tally;
pub use tile::Tile;
pub use title_bar::TitleBar;
pub use window::{
    FRAME_RADIUS, SHADOW_MARGIN, WINDOW_SHADOW, Window, framed, resize_edges, set_floating,
    set_shadow_outside, shadow_margin, shadow_outside, shadowed,
};
