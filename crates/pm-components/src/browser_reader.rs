//! Reader view: the article of a page, taken out of it by the helper, written as a document of its own in the colours of the app's theme, which the browser shows sealed from the network in place of the page until the reader goes back to it.

use crate::theme;
use crate::theme::Role;
use rustle::Color;

/// The article of a page, as the helper took it out for reader view.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReaderArticle {
    /// The address of the page it was taken from.
    pub url: String,
    /// Its title.
    pub title: String,
    /// Who wrote it, or nothing.
    pub byline: String,
    /// The name of the site, or nothing.
    pub site: String,
    /// The language of the page, such as `en`, or nothing.
    pub lang: String,
    /// The direction of its text, `ltr` or `rtl`, or nothing.
    pub dir: String,
    /// Its content as HTML, cleaned of scripts and styles by the helper.
    pub html: String,
}

/// `text` with the characters HTML gives a meaning written as references, so it reads as text in a document or an attribute.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// `color` written as CSS.
fn css(color: Color) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        color.r,
        color.g,
        color.b,
        f32::from(color.a) / 255.0
    )
}

/// The language tag `lang` when it is one, letters, digits and hyphens only, else nothing.
fn language(lang: &str) -> &str {
    let usable = !lang.is_empty()
        && lang.len() <= 35
        && lang
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if usable { lang } else { "" }
}

/// The text direction `dir` when it is `ltr` or `rtl`, else `auto`.
fn direction(dir: &str) -> &str {
    match dir {
        "ltr" | "rtl" => dir,
        _ => "auto",
    }
}

/// The styles of reader view, in the colours of the theme the app is drawn with: the page's background and text, muted text for what is said about the article, the accent for links, and surfaces and borders for code, quotes and tables. Pictures, media and frames are hidden, since a sealed page cannot load them.
fn styles() -> String {
    let background = css(theme::color(Role::Background));
    let text = css(theme::color(Role::Text));
    let muted = css(theme::color(Role::Muted));
    let accent = css(theme::color(Role::Accent));
    let surface = css(theme::color(Role::Surface));
    let border = css(theme::color(Role::Border));
    format!(
        "html {{ background: {background}; color: {text}; }}\n\
         body {{ margin: 0; padding: 40px 24px 80px; font: 18px/1.65 system-ui, -apple-system, 'Segoe UI', sans-serif; }}\n\
         main {{ max-width: 42em; margin: 0 auto; }}\n\
         header {{ border-bottom: 1px solid {border}; margin-bottom: 1.6em; padding-bottom: 1em; }}\n\
         header h1 {{ font-size: 2em; line-height: 1.2; margin: 0 0 0.4em; }}\n\
         .about {{ color: {muted}; font-size: 0.85em; margin: 0.2em 0; overflow-wrap: anywhere; }}\n\
         a {{ color: {accent}; }}\n\
         h1, h2, h3, h4 {{ line-height: 1.3; }}\n\
         pre, code {{ font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace; font-size: 0.85em; background: {surface}; border-radius: 4px; }}\n\
         code {{ padding: 0.1em 0.3em; }}\n\
         pre {{ padding: 0.8em 1em; overflow-x: auto; border: 1px solid {border}; }}\n\
         pre code {{ padding: 0; background: none; }}\n\
         blockquote {{ margin: 1em 0; padding: 0.2em 1em; border-left: 3px solid {border}; color: {muted}; }}\n\
         table {{ border-collapse: collapse; display: block; overflow-x: auto; }}\n\
         th, td {{ border: 1px solid {border}; padding: 0.3em 0.6em; }}\n\
         hr {{ border: 0; border-top: 1px solid {border}; }}\n\
         img, picture, video, audio, iframe, svg, object, embed {{ display: none; }}\n\
         figure {{ margin: 1em 0; }}\n\
         figcaption {{ color: {muted}; font-size: 0.85em; }}\n"
    )
}

/// The document reader view shows for `article`: its title, who wrote it, the site and the address it came from, then its content, in [`styles`].
pub fn reader_document(article: &ReaderArticle) -> String {
    let title = if article.title.trim().is_empty() {
        "Article"
    } else {
        article.title.trim()
    };
    let about: String = [
        article.byline.trim(),
        article.site.trim(),
        article.url.trim(),
    ]
    .into_iter()
    .filter(|line| !line.is_empty())
    .map(|line| format!("<p class=\"about\">{}</p>", escaped(line)))
    .collect();
    let lang = language(&article.lang);
    let lang = if lang.is_empty() {
        String::new()
    } else {
        format!(" lang=\"{lang}\"")
    };
    format!(
        "<!doctype html><html{lang} dir=\"{dir}\"><head><meta charset=\"utf-8\"><title>{title}</title><style>{styles}</style></head><body><main><header><h1>{title}</h1>{about}</header><article>{content}</article></main></body></html>",
        dir = direction(&article.dir),
        title = escaped(title),
        styles = styles(),
        content = article.html,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The document carries the article's content as it is, its title and what is said about it as text, and only a language and a direction that are usable.
    #[test]
    fn the_reader_document_holds_the_article_safely() {
        let document = reader_document(&ReaderArticle {
            url: "https://example.com/a".to_owned(),
            title: "Fish <&> chips".to_owned(),
            byline: "\"Ann\"".to_owned(),
            lang: "en\" onload=\"x".to_owned(),
            dir: "rtl".to_owned(),
            html: "<p>Body</p>".to_owned(),
            ..ReaderArticle::default()
        });
        assert!(document.contains("<title>Fish &lt;&amp;&gt; chips</title>"));
        assert!(document.contains("<p class=\"about\">&quot;Ann&quot;</p>"));
        assert!(document.contains("<article><p>Body</p></article>"));
        assert!(document.contains("dir=\"rtl\""));
        assert!(!document.contains("onload"));
        assert!(document.starts_with("<!doctype html><html dir="));
    }

    /// An article without a title is called so, and a direction that is neither way is left to the text.
    #[test]
    fn an_untitled_article_has_a_title_and_its_direction_is_left_to_the_text() {
        let document = reader_document(&ReaderArticle {
            dir: "sideways".to_owned(),
            ..ReaderArticle::default()
        });
        assert!(document.contains("<h1>Article</h1>"));
        assert!(document.contains("dir=\"auto\""));
    }
}
