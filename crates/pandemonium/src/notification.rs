//! Nonmodal installation cards assembled from shared UI controls.

use pm_gfx::{Rect, Size};
use pm_ui::{
    Div, Element, Font, IconName, LayoutContext, PaintContext, Styled, TextSize, Theme, button,
    h_flex, icon, paragraph, scroll_area, text, v_flex,
};

use crate::message::Message;
use crate::notice::{Installation, InstallationStage, NotificationAction};

/// One card, measured against the window before its message viewport is built.
pub struct InstallationCard {
    /// The installation state shared with the application.
    card: Installation,
    /// Position among the individually reachable cards.
    position: usize,
    /// Number of held cards.
    count: usize,
    /// Maximum card width within the window.
    width: f32,
    /// Height available above the status bar and below the title bar.
    height: f32,
    /// The theme used to build the card.
    theme: Theme,
    /// The laid-out card, rebuilt after measuring its wrapped message.
    body: Div<Message>,
}

/// Builds a bottom-right installation card without a modal backdrop.
pub fn installation(
    theme: &Theme,
    card: &Installation,
    position: usize,
    count: usize,
    width: f32,
    height: f32,
) -> InstallationCard {
    InstallationCard {
        card: card.clone(),
        position,
        count,
        width,
        height,
        theme: *theme,
        body: v_flex(),
    }
}

impl InstallationCard {
    /// Addresses a control to this card's stable identity.
    fn action(&self, action: NotificationAction) -> Message {
        Message::ActOnNotification(self.card.id, action)
    }

    /// Builds the header with its information icon and separate settings and close controls.
    fn header(&self) -> Div<Message> {
        h_flex()
            .w_full()
            .gap(2)
            .items_center()
            .child(icon(IconName::Info).color(self.theme.colors.link))
            .child(text("Language server").text_sm().flex_1())
            .child(pm_ui::notification_control(
                &self.theme,
                IconName::Settings,
                "Installation preference: Ask / Always / Never",
                self.action(NotificationAction::Preference),
            ))
            .child(pm_ui::notification_control(
                &self.theme,
                IconName::Close,
                "Dismiss notification",
                self.action(NotificationAction::Close),
            ))
    }

    /// Builds explicit install and language-settings actions, stacked in narrow windows.
    fn actions(&self) -> Div<Message> {
        let mut actions = if self.width < 400.0 {
            v_flex()
        } else {
            h_flex()
        }
        .w_full()
        .gap(2);
        let label = match self.card.stage {
            InstallationStage::Offer => Some("Install"),
            InstallationStage::Failed => Some("Retry"),
            InstallationStage::Progress | InstallationStage::Done => None,
        };
        if let Some(label) = label {
            actions =
                actions.child(button(label, self.action(NotificationAction::Install)).filled());
        }
        actions.child(button(
            "Language Settings",
            self.action(NotificationAction::LanguageSettings),
        ))
    }

    /// Builds navigation that keeps every pending offer individually reachable.
    fn navigation(&self) -> Div<Message> {
        h_flex()
            .w_full()
            .gap(2)
            .items_center()
            .child(pm_ui::notification_control(
                &self.theme,
                IconName::ArrowLeft,
                "Previous notification",
                self.action(NotificationAction::Previous),
            ))
            .child(
                text(format!("{} / {}", self.position, self.count))
                    .text_sm()
                    .flex_1(),
            )
            .child(pm_ui::notification_control(
                &self.theme,
                IconName::ArrowRight,
                "Next notification",
                self.action(NotificationAction::Next),
            ))
    }
}

impl Element<Message> for InstallationCard {
    /// Wraps the full message and caps its scrollable viewport to the available window height.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.width = self.width.min(available.width);
        let content_width = (self.width - 24.0).max(1.0);
        let offer = Size::new(content_width, self.height);
        let mut header = self.header();
        let mut actions = self.actions();
        let mut navigation = self.navigation();
        let navigation_height = if self.count > 1 {
            navigation.measure(offer, cx).height + 8.0
        } else {
            0.0
        };
        let controls_height = header.measure(offer, cx).height
            + actions.measure(offer, cx).height
            + navigation_height
            + self.theme.text.xs.line_height
            + 48.0;
        let mut message = paragraph()
            .break_long_words()
            .span(
                &self.card.text,
                Font::new(TextSize::Base),
                self.theme.colors.text,
            )
            .w_full();
        let message_height = message
            .measure(offer, cx)
            .height
            .min((self.height - controls_height).max(1.0));
        let id = self.card.id;
        self.body = v_flex()
            .w_px(self.width)
            .p(3)
            .gap(2)
            .rounded(self.theme.radius.md)
            .border_1(self.theme.colors.border)
            .bg(self.theme.colors.surface)
            .child(header)
            .child(
                scroll_area(self.card.scroll.clone(), message)
                    .w_full()
                    .h_px(message_height)
                    .with_scrollbar(move |event, step| {
                        Message::ScrollNotification(id, event, step)
                    }),
            )
            .child(actions)
            .when(self.count > 1, |body| body.child(navigation))
            .child(
                paragraph()
                    .break_long_words()
                    .span(
                        "F6 focus · Tab move · Enter activate · Esc return",
                        Font::new(TextSize::Xs),
                        self.theme.colors.text_muted,
                    )
                    .w_full(),
            );
        self.body.measure(available, cx)
    }

    /// Blocks clicks through the card body while leaving editing elsewhere usable.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, Message>) {
        self.card.bounds.set(bounds);
        cx.clickable(bounds, None, None);
        self.body.paint(bounds, cx);
    }
}
