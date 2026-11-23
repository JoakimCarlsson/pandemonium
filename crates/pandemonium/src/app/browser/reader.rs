//! Reader view in the window: the article the helper took out of a tab's page, shown as a sealed page styled like the app in place of the page until the reader goes back, the page goes elsewhere or the tab closes; and files dropped on a page.

use pm_browser::View;
use pm_browser::chrome::{Article, PageAction};
use pm_components::areas::BrowserEvent;
use pm_components::browser_reader::{ReaderArticle, reader_document};
use pm_components::exhibit::PREVIEW_NEEDS_UPDATE;
use pm_components::target::Target;

use super::super::App;

/// The article of a tab shown in reader view.
pub(super) struct Reader {
    /// The sealed page that shows it.
    pub(super) view: View,
    /// The address of the page it was taken from; the reader view closes once the page is elsewhere.
    url: String,
    /// The size the page was last given, in logical pixels, and the scale.
    sized: Option<(u32, u32, f32)>,
}

/// The pane's form of an article the helper took out of a page.
fn reader_article(article: &Article) -> ReaderArticle {
    let html = if article.error.is_empty() {
        article.html.clone()
    } else {
        format!(
            "<p>{}</p>",
            article
                .error
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        )
    };
    ReaderArticle {
        url: article.url.clone(),
        title: article.title.clone(),
        byline: article.byline.clone(),
        site: article.site.clone(),
        lang: article.lang.clone(),
        dir: article.dir.clone(),
        html,
    }
}

impl App {
    /// The view the tab `tab` shows: its article while reader view shows it, else its page.
    pub(super) fn browser_front(&self, tab: u32) -> Option<&View> {
        self.web
            .readers
            .get(&tab)
            .map(|reader| &reader.view)
            .or_else(|| self.web.pages.get(&tab).map(|shown| &shown.view))
    }

    /// Has the page of tab `tab` hand its next frame to the window whole, as when what the tab shows changes between the page and its article.
    fn browser_repaint(&mut self, tab: u32) {
        if let Some(shown) = self.web.pages.get_mut(&tab) {
            shown.shown = 0;
            shown.picture = 0;
        }
    }

    /// Shows the page of tab `tab` again in place of its article, when reader view shows it, and says whether it did.
    pub(super) fn browser_close_reader(&mut self, tab: u32) -> bool {
        if self.web.readers.remove(&tab).is_none() {
            return false;
        }
        self.browser_repaint(tab);
        self.model.update(|state| {
            state.browser_scoped_to_tab(tab, |state| state.browser_reading(tab, false));
        });
        true
    }

    /// Shows the page of tab `tab` again when reader view shows its article, or else asks its page for the article, which [`Self::browser_article`] shows once it comes.
    pub(super) fn browser_toggle_reader(&mut self, tab: u32) {
        if self.browser_close_reader(tab) {
            return;
        }
        if let Some(shown) = self.web.pages.get(&tab).filter(|shown| !shown.sealed) {
            shown.view.act(PageAction::ReadArticle);
        }
    }

    /// Shows `article`, which the page of tab `tab` reported, in reader view in place of the page, styled like the app; one that came with an error says it there.
    pub(super) fn browser_article(&mut self, tab: u32, article: Article) {
        let current = self.web.pages.get(&tab).map(|shown| shown.view.address());
        if current.as_deref() != Some(article.url.as_str()) || self.web.readers.contains_key(&tab) {
            return;
        }
        let document = reader_document(&reader_article(&article));
        let size = self.browser_size(Target::Browser(BrowserEvent::BrowserBody), self.scale);
        let Some(engine) = self.browser_engine() else {
            return;
        };
        if !engine.sealed_pages() {
            self.model.update(|state| {
                state.browser_tab_notice(
                    tab,
                    &format!(
                        "{PREVIEW_NEEDS_UPDATE}: the installed browser cannot show reader view."
                    ),
                );
            });
            return;
        }
        let title = article.title.clone();
        let Ok(view) = View::open_html(engine, &document, &title, size) else {
            return;
        };
        self.web.readers.insert(
            tab,
            Reader {
                view,
                url: article.url,
                sized: Some(size),
            },
        );
        self.browser_repaint(tab);
        self.model.update(|state| {
            state.browser_scoped_to_tab(tab, |state| state.browser_reading(tab, true));
        });
    }

    /// Closes reader view in each tab whose page went elsewhere or away, and gives the reader view shown the size of the room the pane has for the page.
    pub(super) fn browser_readers(&mut self, scale: f32) {
        let gone: Vec<u32> = self
            .web
            .readers
            .iter()
            .filter(|(tab, reader)| {
                self.web
                    .pages
                    .get(tab)
                    .is_none_or(|shown| shown.view.address() != reader.url)
            })
            .map(|(tab, _)| *tab)
            .collect();
        for tab in gone {
            self.browser_close_reader(tab);
        }
        let page = Target::Browser(BrowserEvent::BrowserBody);
        let size = self.browser_size(page, scale);
        if self.browser_laid(page).is_some()
            && let Some(active) = self.browser_active()
            && let Some(reader) = self.web.readers.get_mut(&active)
            && reader.sized != Some(size)
        {
            reader.sized = Some(size);
            reader.view.resize(size.0, size.1, size.2);
        }
    }

    /// Drops the files at `paths` on the page shown, at `at` in logical pixels from its top left, as letting go of them there in a file manager's drag does; a tab that shows an article or a sealed document takes no files.
    pub(in crate::app) fn browser_files_dropped(&mut self, paths: Vec<String>, at: (f32, f32)) {
        let Some(active) = self.browser_active() else {
            return;
        };
        if paths.is_empty() || self.web.readers.contains_key(&active) {
            return;
        }
        if let Some(shown) = self.web.pages.get(&active).filter(|shown| !shown.sealed) {
            shown.view.act(PageAction::DropFiles {
                x: at.0 as i32,
                y: at.1 as i32,
                paths,
            });
            self.focus
                .request_focus(Target::Browser(BrowserEvent::BrowserBody));
        }
    }
}
