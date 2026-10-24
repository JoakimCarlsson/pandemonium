//! Long inputs retain navigation and composition while building only visible lines.

use pm_renderer::DrawList;
use pm_text::{Edit, Motion};
use pm_ui::{Input as FrameInput, Output, draw};

use super::*;

/// Draws a multiline input in a small viewport with keyboard focus.
fn paint(field: &Field, shaper: &mut Shaper) -> Output {
    let id = FocusId(999);
    let root = Input::new(field, id)
        .variant(Variant::Multiline)
        .wrap(300.0, shaper)
        .build();
    draw(
        &root,
        340.0,
        240.0,
        &FrameInput {
            focused: Some(id),
            ..FrameInput::default()
        },
        &pm_theme::default_family().dark,
        shaper,
        &mut DrawList::new(),
    )
}

/// Scrolling thousands of lines does not create thousands of shaped field boxes.
#[test]
fn virtual_lines_keep_keyboard_navigation_and_composition() {
    let mut shaper = Shaper::new();
    let mut field =
        Field::new("Unicode café 🦀 in an editable paragraph.\n".repeat(1500)).multiline(true);
    for scroll in [0.0, 700.0, 1_000_000.0] {
        field.apply(Edit::ScrollTo(scroll));
        let output = paint(&field, &mut shaper);
        assert!(output.fields.len() <= MAX_LINES + 3);
        let layout = &output.field_layouts[0];
        assert!(layout.lines.len() > 1500);
        assert_eq!(&*layout.content, field.text());
        let line = layout.lines[50];
        assert_eq!(
            output.line_edge(FocusId(999), line.start, false),
            Some(line.start)
        );
        assert_eq!(
            output.line_edge(FocusId(999), line.start, true),
            Some(line.end)
        );
        assert_eq!(
            output.vertical(FocusId(999), line.start, 1, &mut shaper),
            Some(layout.lines[51].start)
        );
    }
    field.apply(Edit::Move(Motion::End, false));
    field.apply(Edit::Compose("字".to_owned()));
    let output = paint(&field, &mut shaper);
    let caret = field.to_shown(field.caret());
    assert!(output.field_layouts[0].content.ends_with('字'));
    assert!(
        output
            .fields
            .iter()
            .any(|line| line.start <= caret && caret <= line.start + line.length)
    );
    field.apply(Edit::Compose(String::new()));
    field.apply(Edit::SelectAll);
    assert_eq!(field.selected(), field.text());
}

/// Density and text scaling preserve the caret and keep the viewport proportional to its actual font.
#[test]
fn virtual_lines_follow_font_scale() {
    let mut shaper = Shaper::new();
    let field = Field::new("large text with Unicode 🦀\n".repeat(100)).multiline(true);
    for density in [0.8, 1.0] {
        for text_scale in [1.0, 1.5] {
            pm_style::set_scales(density, text_scale);
            let output = paint(&field, &mut shaper);
            assert!(output.fields.len() <= MAX_LINES + 3);
            assert!(output.fields.iter().any(
                |line| line.start <= field.caret() && field.caret() <= line.start + line.length
            ));
        }
    }
    pm_style::set_scales(1.0, 1.0);
}
