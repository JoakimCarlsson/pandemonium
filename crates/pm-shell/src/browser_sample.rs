//! A browser with made-up pages for the snapshots: a painted page, the find bar, the menu, the developer tools, a page's question and a download, an untrusted certificate, the question before clearing site data, and a page that cannot be shown.

use std::sync::Arc;
use std::time::Instant;

use pm_browser::{CertificateInfo, DownloadState, Frame, Notice};

use crate::browser::{Browser, BrowserMenu, PageSearch, Picture};

/// Which state of the browser a snapshot shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserSample {
    /// A painted page among a few tabs, one of them an agent's and one locked.
    Page,
    /// The find bar over the page, with matches counted.
    Find,
    /// The menu over the page.
    Menu,
    /// The developer tools beside the page.
    Tools,
    /// A page asking for the camera, and a finished download.
    Ask,
    /// A site whose certificate is not trusted.
    Certificate,
    /// The question before clearing cookies and site data.
    Clear,
    /// A page that cannot be shown because the browser is not installed.
    Notice,
    /// A painted page while the reader picks an element of it for the prompt.
    Pick,
}

/// A made-up picture `width` by `height` pixels: a header in `accent`, a column of text lines and a card, on white.
fn painted(width: u32, height: u32, accent: [u8; 3], generation: u64) -> Picture {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let header = y < 56;
            let line = (96..height.saturating_sub(120)).contains(&y)
                && (y - 96) % 28 < 10
                && x > 48
                && x < 48 + (width / 2).saturating_sub((y % 5) * 40);
            let card = x > width * 3 / 5 && x < width - 48 && (96..300).contains(&y);
            let pixel = if header {
                accent
            } else if card {
                [236, 240, 247]
            } else if line {
                [200, 204, 212]
            } else {
                [255, 255, 255]
            };
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
    }
    Picture(Frame {
        width,
        height,
        generation,
        rgba: Arc::new(rgba),
    })
}

/// The browser `sample` names.
pub fn sample_browser(sample: BrowserSample) -> Browser {
    let mut browser = Browser::new("http://localhost:5173/", None);
    browser.open_tab("http://localhost:5173/docs", None, true, false);
    browser.open_tab("https://example.com/", None, false, false);
    browser.take_commands();
    browser.tabs[0].title = "Vite + React".to_owned();
    browser.tabs[1].title = "Docs".to_owned();
    browser.tabs[2].locked = true;
    browser.tabs[0].page = Some(painted(1100, 720, [100, 108, 255], 1));
    let now = Instant::now();
    match sample {
        BrowserSample::Page => {}
        BrowserSample::Find => {
            browser.tabs[0].search = Some(PageSearch {
                query: "count".to_owned(),
                active: 2,
                total: 5,
                counted: true,
            });
        }
        BrowserSample::Menu => browser.menu = Some(BrowserMenu { at: (220.0, 90.0) }),
        BrowserSample::Tools => {
            browser.tools = true;
            browser.tools_width = 420.0;
            browser.tools_page = Some(painted(420, 720, [41, 42, 45], 2));
        }
        BrowserSample::Ask => {
            let safety = &mut browser.tabs[0].safety;
            safety.take_notice(
                Notice::Permission {
                    id: 1,
                    origin: "http://localhost:5173".to_owned(),
                    permission: "camera".to_owned(),
                },
                now,
            );
            safety.take_notice(
                Notice::Download {
                    name: "report.pdf".to_owned(),
                    state: DownloadState::Finished,
                },
                now,
            );
        }
        BrowserSample::Certificate => {
            browser.tabs[0].safety.take_notice(
                Notice::Certificate(Box::new(CertificateInfo {
                    url: "https://self-signed.local:8443/".to_owned(),
                    error: "ERR_CERT_AUTHORITY_INVALID".to_owned(),
                    subject: "self-signed.local".to_owned(),
                    issuer: "self-signed.local".to_owned(),
                    fingerprint: "3F:9A:1C:77:0B:42:DE:91:5A:6E:2B:C4:88:10:F3:7D".to_owned(),
                    expires: "2027-03-01".to_owned(),
                })),
                now,
            );
        }
        BrowserSample::Clear => browser.clearing = true,
        BrowserSample::Pick => browser.tabs[0].picking = true,
        BrowserSample::Notice => {
            browser.tabs[0].page = None;
            browser.tabs[0].notice = Some(
                "The embedded browser is not installed. It is downloaded when you turn it on in Settings under Browser.".to_owned(),
            );
        }
    }
    browser
}
