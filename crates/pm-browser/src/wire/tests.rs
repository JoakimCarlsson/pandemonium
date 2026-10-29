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
