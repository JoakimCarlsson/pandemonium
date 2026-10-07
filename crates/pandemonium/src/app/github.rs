//! Browsing GitHub organizations and repositories through the clone picker.

use crate::app::App;
use crate::picker::{Choice, Kind, Row};

impl App {
    /// Discovers GitHub accounts away from the window for this picker opening.
    pub(super) fn ask_github_owners(&self) {
        self.ask_git_later(Kind::CloneSources, || {
            let mut rows = vec![url_row()];
            match pm_core::github_owners() {
                Ok(owners) => rows.extend(owners.into_iter().map(|owner| {
                    Row {
                        section: None,
                        label: owner.login.clone(),
                        detail: if owner.organization {
                            "Organization"
                        } else {
                            "Personal repositories"
                        }
                        .to_owned(),
                        choice: Choice::GithubOwner(owner.login, owner.organization),
                        enabled: true,
                    }
                })),
                Err(trouble) => {
                    rows.push(status_row(&trouble));
                    rows.push(Row {
                        section: None,
                        label: "Retry GitHub discovery".to_owned(),
                        detail: "Sign in with gh auth login".to_owned(),
                        choice: Choice::CloneSources,
                        enabled: true,
                    });
                }
            }
            rows
        });
    }

    /// Opens a searchable repository list and gathers all pages in the background.
    pub(super) fn open_github_repositories(&mut self, owner: pm_core::GithubOwner) {
        self.open_picker_with(
            Kind::CloneRepositories,
            vec![back_row(), status_row("Loading repositories…")],
            String::new(),
        );
        self.ask_git_later(Kind::CloneRepositories, move || {
            let mut rows = vec![back_row()];
            match pm_core::github_repositories(&owner) {
                Ok(repositories) => {
                    let ssh = pm_core::github_uses_ssh();
                    if repositories.is_empty() {
                        rows.push(status_row("No repositories available for this account"));
                    }
                    rows.extend(repositories.into_iter().map(|repository| Row {
                        section: None,
                        label: repository.full_name,
                        detail: format!(
                            "{}{}",
                            if repository.private {
                                "Private · "
                            } else {
                                ""
                            },
                            repository.description.unwrap_or_default()
                        ),
                        choice: Choice::CloneRepository(if ssh {
                            repository.ssh_url
                        } else {
                            repository.clone_url
                        }),
                        enabled: true,
                    }));
                }
                Err(trouble) => {
                    rows.push(status_row(&trouble));
                    rows.push(Row {
                        section: None,
                        label: "Retry loading repositories".to_owned(),
                        detail: owner.login.clone(),
                        choice: Choice::GithubOwner(owner.login, owner.organization),
                        enabled: true,
                    });
                }
            }
            rows
        });
    }
}

/// Offers URL entry immediately while GitHub discovery is running.
pub(super) fn source_rows() -> Vec<Row> {
    vec![
        url_row(),
        status_row("Loading GitHub accounts and organizations…"),
    ]
}

/// The choice that keeps cloning from arbitrary repository URLs available.
fn url_row() -> Row {
    Row {
        section: None,
        label: "Enter repository URL…".to_owned(),
        detail: "HTTPS or SSH".to_owned(),
        choice: Choice::CloneUrl,
        enabled: true,
    }
}

/// Returns from a repository list to account selection.
fn back_row() -> Row {
    Row {
        section: None,
        label: "Back to GitHub accounts and organizations…".to_owned(),
        detail: String::new(),
        choice: Choice::CloneSources,
        enabled: true,
    }
}

/// Shows progress, an empty result, or a CLI failure without offering an action.
fn status_row(message: &str) -> Row {
    Row {
        section: None,
        label: message.lines().next().unwrap_or(message).to_owned(),
        detail: String::new(),
        choice: Choice::CloneSources,
        enabled: false,
    }
}
