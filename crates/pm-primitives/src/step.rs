//! Numbers a style function takes, and the spacing step they may count in.

/// Logical pixels in one step of the spacing scale.
pub const STEP_PX: f32 = 4.0;

/// A plain number given to a style function: `4` is four steps of the spacing
/// scale, sixteen pixels, wherever a function counts steps, and four pixels,
/// or four percent, wherever it takes raw units.
pub trait Number: Copy {
    /// The number itself, in whatever unit the receiving function takes.
    fn value(self) -> f32;

    /// The number counted in steps of the spacing scale, in logical pixels.
    fn step_pixels(self) -> f32 {
        self.value() * STEP_PX
    }
}

macro_rules! number_for {
    ($($kind:ty),*) => {
        $(
            impl Number for $kind {
                /// The number itself, in whatever unit the receiving function takes.
                fn value(self) -> f32 {
                    self as f32
                }
            }
        )*
    };
}

number_for!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);
