//! Lays a file out with the program a language names, when it names one.

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

    /// Lays the focused file out with the program its language names, when
    /// it names one and the file is laid out on save.
    pub(super) fn format_externally(&mut self) {
        let settings = self.active_language_settings();
        let Formatter::External(line) = settings.formatter else {
            return;
        };
        if !settings.format_on_save {
            return;
        }
        let Some(file) = self.active_file() else {
            return;
        };
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
    }
}
