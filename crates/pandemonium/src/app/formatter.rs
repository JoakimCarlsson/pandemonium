//! External formatting and the Markdown fallback for files without server formatting.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::app::App;
use crate::config::languages::{Formatter, LanguageSettings};

/// How long a formatter has to answer before the file is written as it is.
const PATIENCE: Duration = Duration::from_secs(10);

/// What a formatter's command line has in place of the file's path.
const PATH_WORD: &str = "{path}";

/// Pipes `text` through `line`, run in the directory of `path`, and answers
/// what the program wrote, or why it did not.
///
/// A word of the command line that is `{path}` is the file's path, for the
/// programs that choose a style by the name of what they are given.
fn pipe_through(line: &str, path: &Path, text: &str) -> Result<String, String> {
    let mut words = pm_acp::command_words(line).into_iter();
    let program = words.next().ok_or("No formatter command is set.")?;
    let mut child = Command::new(&program)
        .args(words.map(|word| match word == PATH_WORD {
            true => path.display().to_string(),
            false => word,
        }))
        .current_dir(path.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    let mut stdin = child.stdin.take().ok_or("The formatter has no input.")?;
    let mut stdout = child.stdout.take().ok_or("The formatter has no output.")?;
    let mut stderr = child.stderr.take().ok_or("The formatter has no errors.")?;
    let input = text.to_owned();
    std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut written, mut complained) = (String::new(), String::new());
        let _ = stderr.read_to_string(&mut complained);
        let read = stdout.read_to_string(&mut written);
        let _ = sender.send((read.map(|_| written), complained));
    });
    let Ok((written, complained)) = receiver.recv_timeout(PATIENCE) else {
        let _ = child.kill();
        return Err(format!("{program} took too long."));
    };
    let status = child.wait().map_err(|error| error.to_string())?;
    match (status.success(), written) {
        (true, Ok(written)) => Ok(written),
        _ => Err(format!(
            "{program} failed: {}",
            complained.lines().next().unwrap_or("no reason given")
        )),
    }
}

impl App {
    /// The settings of the language the focused file is written in.
    pub(super) fn active_language_settings(&self) -> LanguageSettings {
        let language = self.active_file().and_then(|file| {
            file.borrow()
                .buffer()
                .language()
                .map(|language| language.name())
        });
        self.preferences.language(language)
    }

    /// Formats locally when an external command is selected or Markdown has no
    /// server offering document formatting; returns whether the command was handled.
    pub(super) fn format_locally(&mut self, on_save: bool) -> bool {
        let settings = self.active_language_settings();
        if on_save && !settings.format_on_save {
            return true;
        }
        let Some(file) = self.active_file() else {
            return true;
        };
        match settings.formatter {
            Formatter::Off => true,
            Formatter::LanguageServer => {
                let reflow = {
                    let file = file.borrow();
                    let buffer = file.buffer();
                    buffer
                        .language()
                        .is_some_and(|language| language.name() == "Markdown")
                        && !file
                            .servers()
                            .iter()
                            .any(|client| client.offers(&pm_text::Request::Format, buffer.path()))
                };
                if reflow {
                    file.borrow_mut()
                        .edit(|buffer| pm_text::reflow_markdown(buffer, settings.line_length));
                }
                reflow
            }
            Formatter::External(line) => {
                let (path, text) = {
                    let file = file.borrow();
                    (file.buffer().path().to_path_buf(), file.buffer().contents())
                };
                match pipe_through(&line, &path, &text) {
                    Ok(laid_out) if laid_out != text => {
                        file.borrow_mut()
                            .edit(|buffer| buffer.set_contents(&laid_out));
                    }
                    Ok(_) => {}
                    Err(error) => self.notices.trouble(error, None),
                }
                true
            }
        }
    }
}
