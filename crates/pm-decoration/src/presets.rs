//! Corner radii, shadow presets and blur radii.

use crate::ShadowLayer;

/// A named corner radius.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Radius {
    /// 2 logical pixels.
    Xs,
    /// 4 logical pixels.
    Sm,
    /// 6 logical pixels.
    Md,
    /// 8 logical pixels.
    Lg,
    /// 12 logical pixels.
    Xl,
    /// 16 logical pixels.
    Xl2,
    /// 24 logical pixels.
    Xl3,
    /// 32 logical pixels.
    Xl4,
}

impl Radius {
    /// The radius in logical pixels.
    pub const fn pixels(self) -> f32 {
        match self {
            Self::Xs => 2.0,
            Self::Sm => 4.0,
            Self::Md => 6.0,
            Self::Lg => 8.0,
            Self::Xl => 12.0,
            Self::Xl2 => 16.0,
            Self::Xl3 => 24.0,
            Self::Xl4 => 32.0,
        }
    }
}

impl Radius {
    /// Every radius, smallest first.
    pub const ALL: [Self; 8] = [
        Self::Xs,
        Self::Sm,
        Self::Md,
        Self::Lg,
        Self::Xl,
        Self::Xl2,
        Self::Xl3,
        Self::Xl4,
    ];
}

/// A named box shadow.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Shadow {
    /// 1 layer.
    Xs2,
    /// 1 layer.
    Xs,
    /// 2 layers.
    Sm,
    /// 2 layers.
    Md,
    /// 2 layers.
    Lg,
    /// 2 layers.
    Xl,
    /// 1 layer.
    Xl2,
}

impl Shadow {
    /// The layers, painted first to last.
    pub const fn layers(self) -> &'static [ShadowLayer] {
        match self {
            Self::Xs2 => const { &[ShadowLayer::new(0.0, 1.0, 0.0, 0.0, 0.05, false)] },
            Self::Xs => const { &[ShadowLayer::new(0.0, 1.0, 2.0, 0.0, 0.05, false)] },
            Self::Sm => {
                const {
                    &[
                        ShadowLayer::new(0.0, 1.0, 3.0, 0.0, 0.1, false),
                        ShadowLayer::new(0.0, 1.0, 2.0, -1.0, 0.1, false),
                    ]
                }
            }
            Self::Md => {
                const {
                    &[
                        ShadowLayer::new(0.0, 4.0, 6.0, -1.0, 0.1, false),
                        ShadowLayer::new(0.0, 2.0, 4.0, -2.0, 0.1, false),
                    ]
                }
            }
            Self::Lg => {
                const {
                    &[
                        ShadowLayer::new(0.0, 10.0, 15.0, -3.0, 0.1, false),
                        ShadowLayer::new(0.0, 4.0, 6.0, -4.0, 0.1, false),
                    ]
                }
            }
            Self::Xl => {
                const {
                    &[
                        ShadowLayer::new(0.0, 20.0, 25.0, -5.0, 0.1, false),
                        ShadowLayer::new(0.0, 8.0, 10.0, -6.0, 0.1, false),
                    ]
                }
            }
            Self::Xl2 => const { &[ShadowLayer::new(0.0, 25.0, 50.0, -12.0, 0.25, false)] },
        }
    }
}

/// A named inset box shadow.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InsetShadow {
    /// 1 layer.
    Xs2,
    /// 1 layer.
    Xs,
    /// 1 layer.
    Sm,
}

impl InsetShadow {
    /// The layers, painted first to last.
    pub const fn layers(self) -> &'static [ShadowLayer] {
        match self {
            Self::Xs2 => const { &[ShadowLayer::new(0.0, 1.0, 0.0, 0.0, 0.05, true)] },
            Self::Xs => const { &[ShadowLayer::new(0.0, 1.0, 1.0, 0.0, 0.05, true)] },
            Self::Sm => const { &[ShadowLayer::new(0.0, 2.0, 4.0, 0.0, 0.05, true)] },
        }
    }
}

/// A named drop shadow cast by the painted shape.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DropShadow {
    /// 1 layer.
    Xs,
    /// 1 layer.
    Sm,
    /// 1 layer.
    Md,
    /// 1 layer.
    Lg,
    /// 1 layer.
    Xl,
    /// 1 layer.
    Xl2,
}

impl DropShadow {
    /// The layers, painted first to last.
    pub const fn layers(self) -> &'static [ShadowLayer] {
        match self {
            Self::Xs => const { &[ShadowLayer::new(0.0, 1.0, 1.0, 0.0, 0.05, false)] },
            Self::Sm => const { &[ShadowLayer::new(0.0, 1.0, 2.0, 0.0, 0.15, false)] },
            Self::Md => const { &[ShadowLayer::new(0.0, 3.0, 3.0, 0.0, 0.12, false)] },
            Self::Lg => const { &[ShadowLayer::new(0.0, 4.0, 4.0, 0.0, 0.15, false)] },
            Self::Xl => const { &[ShadowLayer::new(0.0, 9.0, 7.0, 0.0, 0.1, false)] },
            Self::Xl2 => const { &[ShadowLayer::new(0.0, 25.0, 25.0, 0.0, 0.15, false)] },
        }
    }
}

/// A named text shadow.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextShadow {
    /// 1 layer.
    Xs2,
    /// 1 layer.
    Xs,
    /// 3 layers.
    Sm,
    /// 3 layers.
    Md,
    /// 3 layers.
    Lg,
}

impl TextShadow {
    /// The layers, painted first to last.
    pub const fn layers(self) -> &'static [ShadowLayer] {
        match self {
            Self::Xs2 => const { &[ShadowLayer::new(0.0, 1.0, 0.0, 0.0, 0.15, false)] },
            Self::Xs => const { &[ShadowLayer::new(0.0, 1.0, 1.0, 0.0, 0.2, false)] },
            Self::Sm => {
                const {
                    &[
                        ShadowLayer::new(0.0, 1.0, 0.0, 0.0, 0.075, false),
                        ShadowLayer::new(0.0, 1.0, 1.0, 0.0, 0.075, false),
                        ShadowLayer::new(0.0, 2.0, 2.0, 0.0, 0.075, false),
                    ]
                }
            }
            Self::Md => {
                const {
                    &[
                        ShadowLayer::new(0.0, 1.0, 1.0, 0.0, 0.1, false),
                        ShadowLayer::new(0.0, 1.0, 2.0, 0.0, 0.1, false),
                        ShadowLayer::new(0.0, 2.0, 4.0, 0.0, 0.1, false),
                    ]
                }
            }
            Self::Lg => {
                const {
                    &[
                        ShadowLayer::new(0.0, 1.0, 2.0, 0.0, 0.1, false),
                        ShadowLayer::new(0.0, 3.0, 2.0, 0.0, 0.1, false),
                        ShadowLayer::new(0.0, 4.0, 8.0, 0.0, 0.1, false),
                    ]
                }
            }
        }
    }
}

/// A named blur radius.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Blur {
    /// 4 logical pixels.
    Xs,
    /// 8 logical pixels.
    Sm,
    /// 12 logical pixels.
    Md,
    /// 16 logical pixels.
    Lg,
    /// 24 logical pixels.
    Xl,
    /// 40 logical pixels.
    Xl2,
    /// 64 logical pixels.
    Xl3,
}

impl Blur {
    /// The blur radius in logical pixels.
    pub const fn pixels(self) -> f32 {
        match self {
            Self::Xs => 4.0,
            Self::Sm => 8.0,
            Self::Md => 12.0,
            Self::Lg => 16.0,
            Self::Xl => 24.0,
            Self::Xl2 => 40.0,
            Self::Xl3 => 64.0,
        }
    }
}
