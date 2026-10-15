//! The components the app is built from.
//!
//! A component is a builder. It is made with `new`, given a variant for how
//! it looks and a size for how big it is, and turned into a [`pm_ui::Node`]
//! with `build`; the caller restyles the node with `.styles([...])` and never
//! reaches into it. Each component lives in a module named for what it is,
//! which holds its `Variant` and `Size`, and is re-exported here.

pub mod activity;
pub mod adder;
pub mod age;
pub mod agent_list;
pub mod arrange;
pub mod avatar;
pub mod branch_menu;
pub mod button;
pub mod change_list;
pub mod chat;
pub mod checkbox;
pub mod chip;
pub mod choices;
pub mod code_box;
pub mod composer;
pub mod context_menu;
pub mod diff;
pub mod dock;
pub mod draft;
pub mod file_icon;
pub mod file_tree;
pub mod folder_browser;
pub mod git_controls;
pub mod git_menus;
pub mod git_prompts;
pub mod graph;
pub mod hearing;
pub mod history;
pub mod input;
pub mod inquiry;
pub mod markdown;
pub mod menu;
pub mod message_menu;
pub mod modal;
pub mod parts;
pub mod plan;
pub mod popover;
pub mod progress;
pub mod project;
pub mod queued;
pub mod review;
pub mod sample;
pub mod settings;
pub mod settings_servers;
pub mod sidebar;
pub mod state;
pub mod stepper;
pub mod steps;
pub mod tally;
pub mod terminal;
pub mod terminal_keys;
pub mod tile;
pub mod trail;
pub mod tree;
pub mod usage;
pub mod viewer;
pub mod welcome;

pub use avatar::Avatar;
pub use button::Button;
pub use checkbox::Checkbox;
pub use chip::Chip;
pub use code_box::CodeBox;
pub use context_menu::{
    ContextMenu, MENU_COPY, MENU_CUT, MENU_PASTE, MENU_REDO, MENU_SELECT_ALL, MENU_UNDO,
    is_menu_item,
};
pub use input::Input;
pub use markdown::Markdown;
pub use menu::Menu;
pub use modal::Modal;
pub use popover::Popover;
pub use progress::Progress;
pub use project::Project;
pub use stepper::Stepper;
pub use tally::Tally;
pub use tile::Tile;
