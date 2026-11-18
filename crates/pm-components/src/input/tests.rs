//! Long inputs retain navigation and composition while drawing only visible lines.

use pm_field::{Edit, Motion};

use super::*;
use crate::kit::Report;
use crate::kit::snapshot::lay_out;

/// Lays out a multiline input editing `field` in a small viewport with keyboard focus.
fn paint(field: &Field) -> Report {
    let id = FocusId(999);
    let field = field.clone();
    lay_out(340.0, 240.0, Some(id), move || {
        Input::new(&field, id)
            .variant(Variant::Multiline)
            .wrap(300.0)
            .build();
    })
}

/// Scrolling thousands of lines does not create thousands of shaped field boxes.
#[test]
fn virtual_lines_keep_keyboard_navigation_and_composition() {
    let mut field =
        Field::new("Unicode café 🦀 in an editable paragraph.\n".repeat(1500)).multiline(true);
    for scroll in [0.0, 700.0, 1_000_000.0] {
        field.apply(Edit::ScrollTo(scroll));
        let report = paint(&field);
        assert!(report.fields.len() <= MAX_LINES + 3);
        let layout = &report
            .field(FocusId(999))
            .expect("the field is laid out")
            .layout;
        assert!(layout.lines().len() > 1500);
        assert_eq!(layout.text(), field.text());
        let line = &layout.lines()[50];
        assert_eq!(
            report.line_edge(FocusId(999), line.start, false),
            Some(line.start)
        );
        assert_eq!(
            report.line_edge(FocusId(999), line.start, true),
            Some(line.end)
        );
        assert_eq!(
            report.vertical(FocusId(999), line.start, 1),
            Some(layout.lines()[51].start)
        );
    }
    field.apply(Edit::Move(Motion::End, false));
    field.apply(Edit::Compose("字".to_owned()));
    let report = paint(&field);
    let caret = field.to_shown(field.caret());
    let layout = &report
        .field(FocusId(999))
        .expect("the field is laid out")
        .layout;
    assert!(layout.text().ends_with('字'));
    assert!(caret <= layout.text().len());
    field.apply(Edit::Compose(String::new()));
    field.apply(Edit::SelectAll);
    assert_eq!(field.selected(), field.text());
}

/// Density and text scaling preserve the caret and keep the viewport proportional to its actual font.
#[test]
fn virtual_lines_follow_font_scale() {
    let field = Field::new("large text with Unicode 🦀\n".repeat(100)).multiline(true);
    for density in [0.8, 1.0] {
        for text_scale in [1.0, 1.5] {
            crate::kit::theme::set_scales(density, text_scale);
            let report = paint(&field);
            assert!(report.fields.len() <= MAX_LINES + 3);
            let laid = report.field(FocusId(999)).expect("the field is laid out");
            assert!(field.caret() <= laid.layout.text().len());
            let line = typography().line_height();
            assert!(laid.view.size.height <= line * MAX_LINES as f32 + 1.0);
        }
    }
    crate::kit::theme::set_scales(1.0, 1.0);
}
