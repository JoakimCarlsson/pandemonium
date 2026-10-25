//! The browser engine's process-wide configuration: `EngineApp` sets the switches every browser process starts with.

use cef::*;

wrap_app! {
    pub(crate) struct EngineApp {
        switches: Vec<String>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else {
                return;
            };
            for switch in &self.switches {
                match switch.split_once('=') {
                    Some((name, value)) => command_line.append_switch_with_value(
                        Some(&name.into()),
                        Some(&value.into()),
                    ),
                    None => command_line.append_switch(Some(&switch.as_str().into())),
                }
            }
        }
    }
}
