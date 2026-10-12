//! Responsive breakpoints and named container widths.

/// A viewport width at and above which a variant applies.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Breakpoint {
    /// Applies from 640 logical pixels of viewport width.
    Sm,
    /// Applies from 768 logical pixels of viewport width.
    Md,
    /// Applies from 1024 logical pixels of viewport width.
    Lg,
    /// Applies from 1280 logical pixels of viewport width.
    Xl,
    /// Applies from 1536 logical pixels of viewport width.
    Xl2,
}

impl Breakpoint {
    /// The narrowest viewport, in logical pixels, the breakpoint matches.
    pub const fn min_width(self) -> f32 {
        match self {
            Self::Sm => 640.0,
            Self::Md => 768.0,
            Self::Lg => 1024.0,
            Self::Xl => 1280.0,
            Self::Xl2 => 1536.0,
        }
    }
}

impl Breakpoint {
    /// Every breakpoint, narrowest first.
    pub const ALL: [Self; 5] = [Self::Sm, Self::Md, Self::Lg, Self::Xl, Self::Xl2];
}

/// A named container width.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Container {
    /// 256 logical pixels wide.
    Xs3,
    /// 288 logical pixels wide.
    Xs2,
    /// 320 logical pixels wide.
    Xs,
    /// 384 logical pixels wide.
    Sm,
    /// 448 logical pixels wide.
    Md,
    /// 512 logical pixels wide.
    Lg,
    /// 576 logical pixels wide.
    Xl,
    /// 672 logical pixels wide.
    Xl2,
    /// 768 logical pixels wide.
    Xl3,
    /// 896 logical pixels wide.
    Xl4,
    /// 1024 logical pixels wide.
    Xl5,
    /// 1152 logical pixels wide.
    Xl6,
    /// 1280 logical pixels wide.
    Xl7,
}

impl Container {
    /// The container's width in logical pixels.
    pub const fn width(self) -> f32 {
        match self {
            Self::Xs3 => 256.0,
            Self::Xs2 => 288.0,
            Self::Xs => 320.0,
            Self::Sm => 384.0,
            Self::Md => 448.0,
            Self::Lg => 512.0,
            Self::Xl => 576.0,
            Self::Xl2 => 672.0,
            Self::Xl3 => 768.0,
            Self::Xl4 => 896.0,
            Self::Xl5 => 1024.0,
            Self::Xl6 => 1152.0,
            Self::Xl7 => 1280.0,
        }
    }
}
