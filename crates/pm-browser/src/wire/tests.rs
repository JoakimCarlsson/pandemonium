//! Tests that the messages about the reader's safety and about adding a page to the prompt keep the shapes the helper writes and reads, as `helpers/electron-browser/src/types.js` says.

use super::{CaptureKind, CertificateInfo, DownloadState, Event, Request};

/// The requests the window sends for the reader's safety are serde's default form, as the helper's `types.js` says.
#[test]
fn safety_requests_have_the_shape_the_helper_reads() {
    let origins = serde_json::to_value(Request::AgentOrigins {
        restrict: true,
        origins: vec!["https://example.com".to_owned()],
    })
    .unwrap_or_default();
    assert_eq!(
        origins,
        serde_json::json!({ "AgentOrigins": { "restrict": true, "origins": ["https://example.com"] } })
    );
    let answer = serde_json::to_value(Request::PermissionAnswer {
        page: 1,
        id: 9,
        allow: false,
    })
    .unwrap_or_default();
    assert_eq!(
        answer,
        serde_json::json!({ "PermissionAnswer": { "page": 1, "id": 9, "allow": false } })
    );
    let choice = serde_json::to_value(Request::CertificateChoice {
        page: 1,
        proceed: true,
    })
    .unwrap_or_default();
    assert_eq!(
        choice,
        serde_json::json!({ "CertificateChoice": { "page": 1, "proceed": true } })
    );
}

/// The reports the helper writes for the reader's safety are read back as written.
#[test]
fn safety_reports_are_read_as_the_helper_writes_them() {
    let asked: Event = serde_json::from_str(
        r#"{"PermissionAsked":{"page":1,"id":3,"origin":"https://a.example","permission":"camera"}}"#,
    )
    .unwrap_or(Event::Ready);
    assert!(matches!(asked, Event::PermissionAsked { id: 3, .. }));
    let certificate: Event = serde_json::from_str(
        r#"{"CertificateError":{"page":1,"info":{"url":"https://a.example/","error":"ERR_CERT_DATE_INVALID","subject":"a","issuer":"b","fingerprint":"sha256/x","expires":"2020-01-01"}}}"#,
    )
    .unwrap_or(Event::Ready);
    let Event::CertificateError { info, .. } = certificate else {
        panic!("not a certificate error");
    };
    assert_eq!(
        *info,
        CertificateInfo {
            url: "https://a.example/".to_owned(),
            error: "ERR_CERT_DATE_INVALID".to_owned(),
            subject: "a".to_owned(),
            issuer: "b".to_owned(),
            fingerprint: "sha256/x".to_owned(),
            expires: "2020-01-01".to_owned(),
        }
    );
    let download: Event =
        serde_json::from_str(r#"{"Download":{"page":1,"name":"a.pdf","state":"Finished"}}"#)
            .unwrap_or(Event::Ready);
    assert!(matches!(
        download,
        Event::Download {
            state: DownloadState::Finished,
            ..
        }
    ));
}

/// The requests are written as the helper reads them: the variant's name holding its fields.
#[test]
fn capture_requests_have_the_shapes_of_the_helper() {
    let capture = Request::Capture {
        page: 3,
        kind: CaptureKind::FullPage,
    };
    assert_eq!(
        serde_json::to_string(&capture).unwrap(),
        r#"{"Capture":{"page":3,"kind":"FullPage"}}"#
    );
    let area = Request::PickArea { page: 3, on: true };
    assert_eq!(
        serde_json::to_string(&area).unwrap(),
        r#"{"PickArea":{"page":3,"on":true}}"#
    );
}

/// A report as the helper writes it is read, and one that leaves out fields is too.
#[test]
fn a_captured_report_is_read() {
    let text = r#"{"Captured":{"page":1,"capture":{"kind":"Area","pictures":[{"label":"Visible","path":"/a/pick-1-1.png"},{"label":"Full page","path":"/a/pick-1-2.png"}],"lines":[],"url":"http://x/","title":"T","note":"Area skipped: it was too large to attach.","error":""}}}"#;
    let Event::Captured { page, capture } = serde_json::from_str(text).unwrap() else {
        panic!("not a capture");
    };
    assert_eq!(page, 1);
    assert_eq!(capture.kind, CaptureKind::Area);
    assert_eq!(capture.pictures.len(), 2);
    assert!(capture.note.starts_with("Area skipped"));
    let bare = r#"{"Captured":{"page":1,"capture":{"kind":"Console","lines":["[log] a"]}}}"#;
    let Event::Captured { capture, .. } = serde_json::from_str(bare).unwrap() else {
        panic!("not a capture");
    };
    assert_eq!(capture.lines, ["[log] a"]);
    assert!(capture.error.is_empty());
}

/// The requests for sealed documents are serde's default form, as the helper's `types.js` says, and the report of a render is read as the helper writes it.
#[test]
fn sealed_document_messages_have_the_shape_the_helper_uses() {
    let open = serde_json::to_value(Request::OpenHtml {
        page: 4,
        html: "<p>hi</p>".to_owned(),
        title: "Login".to_owned(),
        width: 800,
        height: 600,
        scale: 1.0,
    })
    .unwrap_or_default();
    assert_eq!(
        open,
        serde_json::json!({ "OpenHtml": { "page": 4, "html": "<p>hi</p>", "title": "Login", "width": 800, "height": 600, "scale": 1.0 } })
    );
    let render = serde_json::to_value(Request::RenderHtml {
        ticket: 7,
        html: "<p>hi</p>".to_owned(),
        width: 1024,
        height: 640,
        path: "/cache/a.png".to_owned(),
        scale: 2.0,
    })
    .unwrap_or_default();
    assert_eq!(
        render,
        serde_json::json!({ "RenderHtml": { "ticket": 7, "html": "<p>hi</p>", "width": 1024, "height": 640, "path": "/cache/a.png", "scale": 2.0 } })
    );
    let rendered: Event =
        serde_json::from_str(r#"{"HtmlRendered":{"ticket":7,"path":"/cache/a.png","error":""}}"#)
            .unwrap_or(Event::Ready);
    assert!(matches!(rendered, Event::HtmlRendered { ticket: 7, .. }));
}

/// The features the helper announces are read as it writes them, and the bare `Ready` an older helper sends alone is still read.
#[test]
fn features_are_read_as_the_helper_writes_them() {
    let announced: Event = serde_json::from_str(r#"{"Features":{"features":["sealed-pages"]}}"#)
        .unwrap_or(Event::Ready);
    let Event::Features { features } = announced else {
        panic!("not the features");
    };
    assert_eq!(features, vec![super::SEALED_PAGES.to_owned()]);
    assert!(matches!(
        serde_json::from_str::<Event>(r#""Ready""#),
        Ok(Event::Ready)
    ));
}

/// What the pane around a page and the helper say to each other reads as the helper writes it: reports nested in `Chrome` events, and actions in `Chrome` requests, a bare string for an action without fields.
#[test]
fn chrome_messages_read_as_the_helper_writes_them() {
    use super::line;
    use crate::chrome::{DownloadProgress, PageAction, PageReport};
    let event: Event = serde_json::from_str(
        r#"{"Chrome":{"page":3,"report":{"Download":{"id":7,"name":"a.zip","received":10,"total":20,"progress":"Running"}}}}"#,
    )
    .unwrap();
    match event {
        Event::Chrome {
            page: 3,
            report: PageReport::Download(item),
        } => {
            assert_eq!(item.id, 7);
            assert_eq!(item.progress, DownloadProgress::Running);
        }
        other => panic!("read as {other:?}"),
    }
    let tooltip: Event =
        serde_json::from_str(r#"{"Chrome":{"page":1,"report":{"Tooltip":{"text":"Hi"}}}}"#)
            .unwrap();
    assert!(matches!(
        tooltip,
        Event::Chrome {
            report: PageReport::Tooltip { x: 0, y: 0, .. },
            ..
        }
    ));
    let print = String::from_utf8(line(&Request::Chrome {
        page: 2,
        action: PageAction::Print,
    }))
    .unwrap();
    assert_eq!(print, "{\"Chrome\":{\"page\":2,\"action\":\"Print\"}}\n");
    let answer = String::from_utf8(line(&Request::Chrome {
        page: 2,
        action: PageAction::AnswerDialog {
            id: 4,
            accept: true,
            text: "x".to_owned(),
        },
    }))
    .unwrap();
    assert!(answer.contains("{\"AnswerDialog\":{\"id\":4,\"accept\":true,\"text\":\"x\"}}"));
}

/// A paste carries its HTML and files only when there are some, an older window's paste without them still reads, and the requests of saving, reader view and dropping files have the shapes the helper reads, as the article report it answers with.
#[test]
fn rich_paste_reader_and_drop_messages_have_the_helpers_shapes() {
    use super::line;
    use crate::chrome::{PageAction, PageReport};
    let rich = String::from_utf8(line(&Request::PasteText {
        page: 1,
        text: "a".to_owned(),
        html: Some("<b>a</b>".to_owned()),
        files: vec!["/x/p.png".to_owned()],
    }))
    .unwrap();
    assert_eq!(
        rich,
        "{\"PasteText\":{\"page\":1,\"text\":\"a\",\"html\":\"<b>a</b>\",\"files\":[\"/x/p.png\"]}}\n"
    );
    let older: Request = serde_json::from_str(r#"{"PasteText":{"page":1,"text":"a"}}"#).unwrap();
    assert!(matches!(
        older,
        Request::PasteText { html: None, ref files, .. } if files.is_empty()
    ));
    let save = String::from_utf8(line(&Request::Chrome {
        page: 2,
        action: PageAction::SavePage,
    }))
    .unwrap();
    assert_eq!(save, "{\"Chrome\":{\"page\":2,\"action\":\"SavePage\"}}\n");
    let drop = String::from_utf8(line(&Request::Chrome {
        page: 2,
        action: PageAction::DropFiles {
            x: 5,
            y: 6,
            paths: vec!["/a".to_owned()],
        },
    }))
    .unwrap();
    assert!(drop.contains("{\"DropFiles\":{\"x\":5,\"y\":6,\"paths\":[\"/a\"]}}"));
    let article: Event = serde_json::from_str(
        r#"{"Chrome":{"page":3,"report":{"Article":{"title":"T","html":"<p>b</p>"}}}}"#,
    )
    .unwrap();
    match article {
        Event::Chrome {
            report: PageReport::Article(article),
            ..
        } => {
            assert_eq!(article.title, "T");
            assert!(article.error.is_empty());
        }
        other => panic!("read as {other:?}"),
    }
    let readable: Event =
        serde_json::from_str(r#"{"Chrome":{"page":3,"report":{"Readable":{"readable":true}}}}"#)
            .unwrap();
    assert!(matches!(
        readable,
        Event::Chrome {
            report: PageReport::Readable { readable: true },
            ..
        }
    ));
}

/// The question for the primary password reads as the helper writes it, its closing as a bare word, and the answer is written as the helper reads it, with `null` for turning it down.
#[test]
fn primary_password_reads_and_writes_as_the_helper_does() {
    use super::line;
    use crate::chrome::{PageAction, PageReport};
    let asked: Event = serde_json::from_str(
        r#"{"Chrome":{"page":4,"report":{"PrimaryPassword":{"create":true,"origin":"https://example.com","problem":""}}}}"#,
    )
    .unwrap();
    match asked {
        Event::Chrome {
            report: PageReport::PrimaryPassword(ask),
            ..
        } => {
            assert!(ask.create);
            assert_eq!(ask.origin, "https://example.com");
        }
        other => panic!("read as {other:?}"),
    }
    let closed: Event =
        serde_json::from_str(r#"{"Chrome":{"page":4,"report":"PrimaryPasswordClosed"}}"#).unwrap();
    assert!(matches!(
        closed,
        Event::Chrome {
            report: PageReport::PrimaryPasswordClosed,
            ..
        }
    ));
    let typed = String::from_utf8(line(&Request::Chrome {
        page: 4,
        action: PageAction::PrimaryPassword {
            password: Some("correct horse".to_owned()),
        },
    }))
    .unwrap();
    assert_eq!(
        typed,
        "{\"Chrome\":{\"page\":4,\"action\":{\"PrimaryPassword\":{\"password\":\"correct horse\"}}}}\n"
    );
    let declined = String::from_utf8(line(&Request::Chrome {
        page: 4,
        action: PageAction::PrimaryPassword { password: None },
    }))
    .unwrap();
    assert!(declined.contains("{\"PrimaryPassword\":{\"password\":null}}"));
}
