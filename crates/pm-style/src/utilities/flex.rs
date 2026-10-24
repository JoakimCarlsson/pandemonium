//! Flex containers and flex items.

use pm_layout::{AlignContent, AlignItems, FlexDirection, FlexRule, FlexWrap, JustifyContent};
use pm_primitives::{Length, Number};

use crate::Utility;

/// Initial main size, in steps.
pub fn basis(steps: impl Number) -> Utility {
    FlexRule::Basis(Length::steps(steps)).into()
}

/// Initial main size as a fraction of the parent, such as `basis_frac(1, 3)`.
pub fn basis_frac(numerator: impl Number, denominator: impl Number) -> Utility {
    FlexRule::Basis(Length::ratio(numerator.value(), denominator.value())).into()
}

/// Position among siblings; lower comes first.
pub fn order(position: i32) -> Utility {
    FlexRule::Order(position).into()
}

/// Growth factor.
pub fn grow_by(factor: impl Number) -> Utility {
    FlexRule::Grow(factor.value()).into()
}

/// Shrink factor.
pub fn shrink_by(factor: impl Number) -> Utility {
    FlexRule::Shrink(factor.value()).into()
}

constants! {
    /// Children left to right.
    flex_row = FlexRule::Direction(FlexDirection::Row);
    /// Children right to left.
    flex_row_reverse = FlexRule::Direction(FlexDirection::RowReverse);
    /// Children top to bottom.
    flex_col = FlexRule::Direction(FlexDirection::Column);
    /// Children bottom to top.
    flex_col_reverse = FlexRule::Direction(FlexDirection::ColumnReverse);
    /// Break into lines.
    flex_wrap = FlexRule::Wrap(FlexWrap::Wrap);
    /// Break into lines, new lines first.
    flex_wrap_reverse = FlexRule::Wrap(FlexWrap::WrapReverse);
    /// Stay on one line.
    flex_nowrap = FlexRule::Wrap(FlexWrap::NoWrap);
    /// Grow and shrink from a zero basis.
    flex_1 = FlexRule::Flex1;
    /// Grow and shrink from the content size.
    flex_auto = FlexRule::FlexAuto;
    /// Shrink from the content size, never grow.
    flex_initial = FlexRule::FlexInitial;
    /// Neither grow nor shrink.
    flex_none = FlexRule::FlexNone;
    /// Take a share of the leftover space.
    grow = FlexRule::Grow(1.0);
    /// Take none of the leftover space.
    grow_0 = FlexRule::Grow(0.0);
    /// Give back overflow.
    shrink = FlexRule::Shrink(1.0);
    /// Give back no overflow.
    shrink_0 = FlexRule::Shrink(0.0);
    /// Main size from the content.
    basis_auto = FlexRule::Basis(Length::Auto);
    /// Main size of the whole parent.
    basis_full = FlexRule::Basis(Length::FULL);
    /// Pack at the main start.
    justify_start = FlexRule::JustifyContent(JustifyContent::Start);
    /// Pack at the main end.
    justify_end = FlexRule::JustifyContent(JustifyContent::End);
    /// Pack in the middle.
    justify_center = FlexRule::JustifyContent(JustifyContent::Center);
    /// Space between items.
    justify_between = FlexRule::JustifyContent(JustifyContent::Between);
    /// Space around items.
    justify_around = FlexRule::JustifyContent(JustifyContent::Around);
    /// Space between and around items.
    justify_evenly = FlexRule::JustifyContent(JustifyContent::Evenly);
    /// Items grow to fill the line.
    justify_stretch = FlexRule::JustifyContent(JustifyContent::Stretch);
    /// The default packing.
    justify_normal = FlexRule::JustifyContent(JustifyContent::Normal);
    /// Items at the cross start.
    items_start = FlexRule::AlignItems(AlignItems::Start);
    /// Items at the cross end.
    items_end = FlexRule::AlignItems(AlignItems::End);
    /// Items centred on the cross axis.
    items_center = FlexRule::AlignItems(AlignItems::Center);
    /// Items on a shared baseline.
    items_baseline = FlexRule::AlignItems(AlignItems::Baseline);
    /// Items stretched across the line.
    items_stretch = FlexRule::AlignItems(AlignItems::Stretch);
    /// Lines packed at the cross start.
    content_start = FlexRule::AlignContent(AlignContent::Start);
    /// Lines packed at the cross end.
    content_end = FlexRule::AlignContent(AlignContent::End);
    /// Lines packed in the middle.
    content_center = FlexRule::AlignContent(AlignContent::Center);
    /// Space between lines.
    content_between = FlexRule::AlignContent(AlignContent::Between);
    /// Space around lines.
    content_around = FlexRule::AlignContent(AlignContent::Around);
    /// Space between and around lines.
    content_evenly = FlexRule::AlignContent(AlignContent::Evenly);
    /// Lines grow to fill the container.
    content_stretch = FlexRule::AlignContent(AlignContent::Stretch);
    /// Follows the container's item alignment.
    self_auto = FlexRule::AlignSelf(None);
    /// This item at the cross start.
    self_start = FlexRule::AlignSelf(Some(AlignItems::Start));
    /// This item at the cross end.
    self_end = FlexRule::AlignSelf(Some(AlignItems::End));
    /// This item centred on the cross axis.
    self_center = FlexRule::AlignSelf(Some(AlignItems::Center));
    /// This item on the baseline.
    self_baseline = FlexRule::AlignSelf(Some(AlignItems::Baseline));
    /// This item stretched across the line.
    self_stretch = FlexRule::AlignSelf(Some(AlignItems::Stretch));
}
