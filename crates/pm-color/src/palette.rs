//! The named colour palette: twenty-five hues in eleven shades each.

use crate::{Oklch, Role, Srgba};

/// A colour: an entry of the named palette or an explicit sRGB value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    /// Pure black.
    Black,
    /// Pure white.
    White,
    /// Fully transparent black.
    Transparent,
    /// oklch(97.1% 0.013 17.38)
    Red50,
    /// oklch(93.6% 0.032 17.717)
    Red100,
    /// oklch(88.5% 0.062 18.334)
    Red200,
    /// oklch(80.8% 0.114 19.571)
    Red300,
    /// oklch(70.4% 0.191 22.216)
    Red400,
    /// oklch(63.7% 0.237 25.331)
    Red500,
    /// oklch(57.7% 0.245 27.325)
    Red600,
    /// oklch(50.5% 0.213 27.518)
    Red700,
    /// oklch(44.4% 0.177 26.899)
    Red800,
    /// oklch(39.6% 0.141 25.723)
    Red900,
    /// oklch(25.8% 0.092 26.042)
    Red950,
    /// oklch(98% 0.016 73.684)
    Orange50,
    /// oklch(95.4% 0.038 75.164)
    Orange100,
    /// oklch(90.1% 0.076 70.697)
    Orange200,
    /// oklch(83.7% 0.128 66.29)
    Orange300,
    /// oklch(75% 0.183 55.934)
    Orange400,
    /// oklch(70.5% 0.213 47.604)
    Orange500,
    /// oklch(64.6% 0.222 41.116)
    Orange600,
    /// oklch(55.3% 0.195 38.402)
    Orange700,
    /// oklch(47% 0.157 37.304)
    Orange800,
    /// oklch(40.8% 0.123 38.172)
    Orange900,
    /// oklch(26.6% 0.079 36.259)
    Orange950,
    /// oklch(98.7% 0.022 95.277)
    Amber50,
    /// oklch(96.2% 0.059 95.617)
    Amber100,
    /// oklch(92.4% 0.12 95.746)
    Amber200,
    /// oklch(87.9% 0.169 91.605)
    Amber300,
    /// oklch(82.8% 0.189 84.429)
    Amber400,
    /// oklch(76.9% 0.188 70.08)
    Amber500,
    /// oklch(66.6% 0.179 58.318)
    Amber600,
    /// oklch(55.5% 0.163 48.998)
    Amber700,
    /// oklch(47.3% 0.137 46.201)
    Amber800,
    /// oklch(41.4% 0.112 45.904)
    Amber900,
    /// oklch(27.9% 0.077 45.635)
    Amber950,
    /// oklch(98.7% 0.026 102.212)
    Yellow50,
    /// oklch(97.3% 0.071 103.193)
    Yellow100,
    /// oklch(94.5% 0.129 101.54)
    Yellow200,
    /// oklch(90.5% 0.182 98.111)
    Yellow300,
    /// oklch(85.2% 0.199 91.936)
    Yellow400,
    /// oklch(79.5% 0.184 86.047)
    Yellow500,
    /// oklch(68.1% 0.162 75.834)
    Yellow600,
    /// oklch(55.4% 0.135 66.442)
    Yellow700,
    /// oklch(47.6% 0.114 61.907)
    Yellow800,
    /// oklch(42.1% 0.095 57.708)
    Yellow900,
    /// oklch(28.6% 0.066 53.813)
    Yellow950,
    /// oklch(98.6% 0.031 120.757)
    Lime50,
    /// oklch(96.7% 0.067 122.328)
    Lime100,
    /// oklch(93.8% 0.127 124.321)
    Lime200,
    /// oklch(89.7% 0.196 126.665)
    Lime300,
    /// oklch(84.1% 0.238 128.85)
    Lime400,
    /// oklch(76.8% 0.233 130.85)
    Lime500,
    /// oklch(64.8% 0.2 131.684)
    Lime600,
    /// oklch(53.2% 0.157 131.589)
    Lime700,
    /// oklch(45.3% 0.124 130.933)
    Lime800,
    /// oklch(40.5% 0.101 131.063)
    Lime900,
    /// oklch(27.4% 0.072 132.109)
    Lime950,
    /// oklch(98.2% 0.018 155.826)
    Green50,
    /// oklch(96.2% 0.044 156.743)
    Green100,
    /// oklch(92.5% 0.084 155.995)
    Green200,
    /// oklch(87.1% 0.15 154.449)
    Green300,
    /// oklch(79.2% 0.209 151.711)
    Green400,
    /// oklch(72.3% 0.219 149.579)
    Green500,
    /// oklch(62.7% 0.194 149.214)
    Green600,
    /// oklch(52.7% 0.154 150.069)
    Green700,
    /// oklch(44.8% 0.119 151.328)
    Green800,
    /// oklch(39.3% 0.095 152.535)
    Green900,
    /// oklch(26.6% 0.065 152.934)
    Green950,
    /// oklch(97.9% 0.021 166.113)
    Emerald50,
    /// oklch(95% 0.052 163.051)
    Emerald100,
    /// oklch(90.5% 0.093 164.15)
    Emerald200,
    /// oklch(84.5% 0.143 164.978)
    Emerald300,
    /// oklch(76.5% 0.177 163.223)
    Emerald400,
    /// oklch(69.6% 0.17 162.48)
    Emerald500,
    /// oklch(59.6% 0.145 163.225)
    Emerald600,
    /// oklch(50.8% 0.118 165.612)
    Emerald700,
    /// oklch(43.2% 0.095 166.913)
    Emerald800,
    /// oklch(37.8% 0.077 168.94)
    Emerald900,
    /// oklch(26.2% 0.051 172.552)
    Emerald950,
    /// oklch(98.4% 0.014 180.72)
    Teal50,
    /// oklch(95.3% 0.051 180.801)
    Teal100,
    /// oklch(91% 0.096 180.426)
    Teal200,
    /// oklch(85.5% 0.138 181.071)
    Teal300,
    /// oklch(77.7% 0.152 181.912)
    Teal400,
    /// oklch(70.4% 0.14 182.503)
    Teal500,
    /// oklch(60% 0.118 184.704)
    Teal600,
    /// oklch(51.1% 0.096 186.391)
    Teal700,
    /// oklch(43.7% 0.078 188.216)
    Teal800,
    /// oklch(38.6% 0.063 188.416)
    Teal900,
    /// oklch(27.7% 0.046 192.524)
    Teal950,
    /// oklch(98.4% 0.019 200.873)
    Cyan50,
    /// oklch(95.6% 0.045 203.388)
    Cyan100,
    /// oklch(91.7% 0.08 205.041)
    Cyan200,
    /// oklch(86.5% 0.127 207.078)
    Cyan300,
    /// oklch(78.9% 0.154 211.53)
    Cyan400,
    /// oklch(71.5% 0.143 215.221)
    Cyan500,
    /// oklch(60.9% 0.126 221.723)
    Cyan600,
    /// oklch(52% 0.105 223.128)
    Cyan700,
    /// oklch(45% 0.085 224.283)
    Cyan800,
    /// oklch(39.8% 0.07 227.392)
    Cyan900,
    /// oklch(30.2% 0.056 229.695)
    Cyan950,
    /// oklch(97.7% 0.013 236.62)
    Sky50,
    /// oklch(95.1% 0.026 236.824)
    Sky100,
    /// oklch(90.1% 0.058 230.902)
    Sky200,
    /// oklch(82.8% 0.111 230.318)
    Sky300,
    /// oklch(74.6% 0.16 232.661)
    Sky400,
    /// oklch(68.5% 0.169 237.323)
    Sky500,
    /// oklch(58.8% 0.158 241.966)
    Sky600,
    /// oklch(50% 0.134 242.749)
    Sky700,
    /// oklch(44.3% 0.11 240.79)
    Sky800,
    /// oklch(39.1% 0.09 240.876)
    Sky900,
    /// oklch(29.3% 0.066 243.157)
    Sky950,
    /// oklch(97% 0.014 254.604)
    Blue50,
    /// oklch(93.2% 0.032 255.585)
    Blue100,
    /// oklch(88.2% 0.059 254.128)
    Blue200,
    /// oklch(80.9% 0.105 251.813)
    Blue300,
    /// oklch(70.7% 0.165 254.624)
    Blue400,
    /// oklch(62.3% 0.214 259.815)
    Blue500,
    /// oklch(54.6% 0.245 262.881)
    Blue600,
    /// oklch(48.8% 0.243 264.376)
    Blue700,
    /// oklch(42.4% 0.199 265.638)
    Blue800,
    /// oklch(37.9% 0.146 265.522)
    Blue900,
    /// oklch(28.2% 0.091 267.935)
    Blue950,
    /// oklch(96.2% 0.018 272.314)
    Indigo50,
    /// oklch(93% 0.034 272.788)
    Indigo100,
    /// oklch(87% 0.065 274.039)
    Indigo200,
    /// oklch(78.5% 0.115 274.713)
    Indigo300,
    /// oklch(67.3% 0.182 276.935)
    Indigo400,
    /// oklch(58.5% 0.233 277.117)
    Indigo500,
    /// oklch(51.1% 0.262 276.966)
    Indigo600,
    /// oklch(45.7% 0.24 277.023)
    Indigo700,
    /// oklch(39.8% 0.195 277.366)
    Indigo800,
    /// oklch(35.9% 0.144 278.697)
    Indigo900,
    /// oklch(25.7% 0.09 281.288)
    Indigo950,
    /// oklch(96.9% 0.016 293.756)
    Violet50,
    /// oklch(94.3% 0.029 294.588)
    Violet100,
    /// oklch(89.4% 0.057 293.283)
    Violet200,
    /// oklch(81.1% 0.111 293.571)
    Violet300,
    /// oklch(70.2% 0.183 293.541)
    Violet400,
    /// oklch(60.6% 0.25 292.717)
    Violet500,
    /// oklch(54.1% 0.281 293.009)
    Violet600,
    /// oklch(49.1% 0.27 292.581)
    Violet700,
    /// oklch(43.2% 0.232 292.759)
    Violet800,
    /// oklch(38% 0.189 293.745)
    Violet900,
    /// oklch(28.3% 0.141 291.089)
    Violet950,
    /// oklch(97.7% 0.014 308.299)
    Purple50,
    /// oklch(94.6% 0.033 307.174)
    Purple100,
    /// oklch(90.2% 0.063 306.703)
    Purple200,
    /// oklch(82.7% 0.119 306.383)
    Purple300,
    /// oklch(71.4% 0.203 305.504)
    Purple400,
    /// oklch(62.7% 0.265 303.9)
    Purple500,
    /// oklch(55.8% 0.288 302.321)
    Purple600,
    /// oklch(49.6% 0.265 301.924)
    Purple700,
    /// oklch(43.8% 0.218 303.724)
    Purple800,
    /// oklch(38.1% 0.176 304.987)
    Purple900,
    /// oklch(29.1% 0.149 302.717)
    Purple950,
    /// oklch(97.7% 0.017 320.058)
    Fuchsia50,
    /// oklch(95.2% 0.037 318.852)
    Fuchsia100,
    /// oklch(90.3% 0.076 319.62)
    Fuchsia200,
    /// oklch(83.3% 0.145 321.434)
    Fuchsia300,
    /// oklch(74% 0.238 322.16)
    Fuchsia400,
    /// oklch(66.7% 0.295 322.15)
    Fuchsia500,
    /// oklch(59.1% 0.293 322.896)
    Fuchsia600,
    /// oklch(51.8% 0.253 323.949)
    Fuchsia700,
    /// oklch(45.2% 0.211 324.591)
    Fuchsia800,
    /// oklch(40.1% 0.17 325.612)
    Fuchsia900,
    /// oklch(29.3% 0.136 325.661)
    Fuchsia950,
    /// oklch(97.1% 0.014 343.198)
    Pink50,
    /// oklch(94.8% 0.028 342.258)
    Pink100,
    /// oklch(89.9% 0.061 343.231)
    Pink200,
    /// oklch(82.3% 0.12 346.018)
    Pink300,
    /// oklch(71.8% 0.202 349.761)
    Pink400,
    /// oklch(65.6% 0.241 354.308)
    Pink500,
    /// oklch(59.2% 0.249 0.584)
    Pink600,
    /// oklch(52.5% 0.223 3.958)
    Pink700,
    /// oklch(45.9% 0.187 3.815)
    Pink800,
    /// oklch(40.8% 0.153 2.432)
    Pink900,
    /// oklch(28.4% 0.109 3.907)
    Pink950,
    /// oklch(96.9% 0.015 12.422)
    Rose50,
    /// oklch(94.1% 0.03 12.58)
    Rose100,
    /// oklch(89.2% 0.058 10.001)
    Rose200,
    /// oklch(81% 0.117 11.638)
    Rose300,
    /// oklch(71.2% 0.194 13.428)
    Rose400,
    /// oklch(64.5% 0.246 16.439)
    Rose500,
    /// oklch(58.6% 0.253 17.585)
    Rose600,
    /// oklch(51.4% 0.222 16.935)
    Rose700,
    /// oklch(45.5% 0.188 13.697)
    Rose800,
    /// oklch(41% 0.159 10.272)
    Rose900,
    /// oklch(27.1% 0.105 12.094)
    Rose950,
    /// oklch(98.4% 0.003 247.858)
    Slate50,
    /// oklch(96.8% 0.007 247.896)
    Slate100,
    /// oklch(92.9% 0.013 255.508)
    Slate200,
    /// oklch(86.9% 0.022 252.894)
    Slate300,
    /// oklch(70.4% 0.04 256.788)
    Slate400,
    /// oklch(55.4% 0.046 257.417)
    Slate500,
    /// oklch(44.6% 0.043 257.281)
    Slate600,
    /// oklch(37.2% 0.044 257.287)
    Slate700,
    /// oklch(27.9% 0.041 260.031)
    Slate800,
    /// oklch(20.8% 0.042 265.755)
    Slate900,
    /// oklch(12.9% 0.042 264.695)
    Slate950,
    /// oklch(98.5% 0.002 247.839)
    Gray50,
    /// oklch(96.7% 0.003 264.542)
    Gray100,
    /// oklch(92.8% 0.006 264.531)
    Gray200,
    /// oklch(87.2% 0.01 258.338)
    Gray300,
    /// oklch(70.7% 0.022 261.325)
    Gray400,
    /// oklch(55.1% 0.027 264.364)
    Gray500,
    /// oklch(44.6% 0.03 256.802)
    Gray600,
    /// oklch(37.3% 0.034 259.733)
    Gray700,
    /// oklch(27.8% 0.033 256.848)
    Gray800,
    /// oklch(21% 0.034 264.665)
    Gray900,
    /// oklch(13% 0.028 261.692)
    Gray950,
    /// oklch(98.5% 0 0)
    Zinc50,
    /// oklch(96.7% 0.001 286.375)
    Zinc100,
    /// oklch(92% 0.004 286.32)
    Zinc200,
    /// oklch(87.1% 0.006 286.286)
    Zinc300,
    /// oklch(70.5% 0.015 286.067)
    Zinc400,
    /// oklch(55.2% 0.016 285.938)
    Zinc500,
    /// oklch(44.2% 0.017 285.786)
    Zinc600,
    /// oklch(37% 0.013 285.805)
    Zinc700,
    /// oklch(27.4% 0.006 286.033)
    Zinc800,
    /// oklch(21% 0.006 285.885)
    Zinc900,
    /// oklch(14.1% 0.005 285.823)
    Zinc950,
    /// oklch(98.5% 0 0)
    Neutral50,
    /// oklch(97% 0 0)
    Neutral100,
    /// oklch(92.2% 0 0)
    Neutral200,
    /// oklch(87% 0 0)
    Neutral300,
    /// oklch(70.8% 0 0)
    Neutral400,
    /// oklch(55.6% 0 0)
    Neutral500,
    /// oklch(43.9% 0 0)
    Neutral600,
    /// oklch(37.1% 0 0)
    Neutral700,
    /// oklch(26.9% 0 0)
    Neutral800,
    /// oklch(20.5% 0 0)
    Neutral900,
    /// oklch(14.5% 0 0)
    Neutral950,
    /// oklch(98.5% 0.001 106.423)
    Stone50,
    /// oklch(97% 0.001 106.424)
    Stone100,
    /// oklch(92.3% 0.003 48.717)
    Stone200,
    /// oklch(86.9% 0.005 56.366)
    Stone300,
    /// oklch(70.9% 0.01 56.259)
    Stone400,
    /// oklch(55.3% 0.013 58.071)
    Stone500,
    /// oklch(44.4% 0.011 73.639)
    Stone600,
    /// oklch(37.4% 0.01 67.558)
    Stone700,
    /// oklch(26.8% 0.007 34.298)
    Stone800,
    /// oklch(21.6% 0.006 56.043)
    Stone900,
    /// oklch(14.7% 0.004 49.25)
    Stone950,
    /// oklch(98.5% 0 0)
    Mauve50,
    /// oklch(96% 0.003 325.6)
    Mauve100,
    /// oklch(92.2% 0.005 325.62)
    Mauve200,
    /// oklch(86.5% 0.012 325.68)
    Mauve300,
    /// oklch(71.1% 0.019 323.02)
    Mauve400,
    /// oklch(54.2% 0.034 322.5)
    Mauve500,
    /// oklch(43.5% 0.029 321.78)
    Mauve600,
    /// oklch(36.4% 0.029 323.89)
    Mauve700,
    /// oklch(26.3% 0.024 320.12)
    Mauve800,
    /// oklch(21.2% 0.019 322.12)
    Mauve900,
    /// oklch(14.5% 0.008 326)
    Mauve950,
    /// oklch(98.8% 0.003 106.5)
    Olive50,
    /// oklch(96.6% 0.005 106.5)
    Olive100,
    /// oklch(93% 0.007 106.5)
    Olive200,
    /// oklch(88% 0.011 106.6)
    Olive300,
    /// oklch(73.7% 0.021 106.9)
    Olive400,
    /// oklch(58% 0.031 107.3)
    Olive500,
    /// oklch(46.6% 0.025 107.3)
    Olive600,
    /// oklch(39.4% 0.023 107.4)
    Olive700,
    /// oklch(28.6% 0.016 107.4)
    Olive800,
    /// oklch(22.8% 0.013 107.4)
    Olive900,
    /// oklch(15.3% 0.006 107.1)
    Olive950,
    /// oklch(98.7% 0.002 197.1)
    Mist50,
    /// oklch(96.3% 0.002 197.1)
    Mist100,
    /// oklch(92.5% 0.005 214.3)
    Mist200,
    /// oklch(87.2% 0.007 219.6)
    Mist300,
    /// oklch(72.3% 0.014 214.4)
    Mist400,
    /// oklch(56% 0.021 213.5)
    Mist500,
    /// oklch(45% 0.017 213.2)
    Mist600,
    /// oklch(37.8% 0.015 216)
    Mist700,
    /// oklch(27.5% 0.011 216.9)
    Mist800,
    /// oklch(21.8% 0.008 223.9)
    Mist900,
    /// oklch(14.8% 0.004 228.8)
    Mist950,
    /// oklch(98.6% 0.002 67.8)
    Taupe50,
    /// oklch(96% 0.002 17.2)
    Taupe100,
    /// oklch(92.2% 0.005 34.3)
    Taupe200,
    /// oklch(86.8% 0.007 39.5)
    Taupe300,
    /// oklch(71.4% 0.014 41.2)
    Taupe400,
    /// oklch(54.7% 0.021 43.1)
    Taupe500,
    /// oklch(43.8% 0.017 39.3)
    Taupe600,
    /// oklch(36.7% 0.016 35.7)
    Taupe700,
    /// oklch(26.8% 0.011 36.5)
    Taupe800,
    /// oklch(21.4% 0.009 43.1)
    Taupe900,
    /// oklch(14.7% 0.004 49.3)
    Taupe950,
    /// An explicit sRGB value outside the palette.
    Srgb(Srgba),
    /// A role, resolved against a theme when painted, at the given opacity
    /// from 0.0 to 1.0.
    Role(Role, f32),
}

impl Color {
    /// Every palette entry, in declaration order.
    pub const PALETTE: [Self; 289] = [
        Self::Black,
        Self::White,
        Self::Transparent,
        Self::Red50,
        Self::Red100,
        Self::Red200,
        Self::Red300,
        Self::Red400,
        Self::Red500,
        Self::Red600,
        Self::Red700,
        Self::Red800,
        Self::Red900,
        Self::Red950,
        Self::Orange50,
        Self::Orange100,
        Self::Orange200,
        Self::Orange300,
        Self::Orange400,
        Self::Orange500,
        Self::Orange600,
        Self::Orange700,
        Self::Orange800,
        Self::Orange900,
        Self::Orange950,
        Self::Amber50,
        Self::Amber100,
        Self::Amber200,
        Self::Amber300,
        Self::Amber400,
        Self::Amber500,
        Self::Amber600,
        Self::Amber700,
        Self::Amber800,
        Self::Amber900,
        Self::Amber950,
        Self::Yellow50,
        Self::Yellow100,
        Self::Yellow200,
        Self::Yellow300,
        Self::Yellow400,
        Self::Yellow500,
        Self::Yellow600,
        Self::Yellow700,
        Self::Yellow800,
        Self::Yellow900,
        Self::Yellow950,
        Self::Lime50,
        Self::Lime100,
        Self::Lime200,
        Self::Lime300,
        Self::Lime400,
        Self::Lime500,
        Self::Lime600,
        Self::Lime700,
        Self::Lime800,
        Self::Lime900,
        Self::Lime950,
        Self::Green50,
        Self::Green100,
        Self::Green200,
        Self::Green300,
        Self::Green400,
        Self::Green500,
        Self::Green600,
        Self::Green700,
        Self::Green800,
        Self::Green900,
        Self::Green950,
        Self::Emerald50,
        Self::Emerald100,
        Self::Emerald200,
        Self::Emerald300,
        Self::Emerald400,
        Self::Emerald500,
        Self::Emerald600,
        Self::Emerald700,
        Self::Emerald800,
        Self::Emerald900,
        Self::Emerald950,
        Self::Teal50,
        Self::Teal100,
        Self::Teal200,
        Self::Teal300,
        Self::Teal400,
        Self::Teal500,
        Self::Teal600,
        Self::Teal700,
        Self::Teal800,
        Self::Teal900,
        Self::Teal950,
        Self::Cyan50,
        Self::Cyan100,
        Self::Cyan200,
        Self::Cyan300,
        Self::Cyan400,
        Self::Cyan500,
        Self::Cyan600,
        Self::Cyan700,
        Self::Cyan800,
        Self::Cyan900,
        Self::Cyan950,
        Self::Sky50,
        Self::Sky100,
        Self::Sky200,
        Self::Sky300,
        Self::Sky400,
        Self::Sky500,
        Self::Sky600,
        Self::Sky700,
        Self::Sky800,
        Self::Sky900,
        Self::Sky950,
        Self::Blue50,
        Self::Blue100,
        Self::Blue200,
        Self::Blue300,
        Self::Blue400,
        Self::Blue500,
        Self::Blue600,
        Self::Blue700,
        Self::Blue800,
        Self::Blue900,
        Self::Blue950,
        Self::Indigo50,
        Self::Indigo100,
        Self::Indigo200,
        Self::Indigo300,
        Self::Indigo400,
        Self::Indigo500,
        Self::Indigo600,
        Self::Indigo700,
        Self::Indigo800,
        Self::Indigo900,
        Self::Indigo950,
        Self::Violet50,
        Self::Violet100,
        Self::Violet200,
        Self::Violet300,
        Self::Violet400,
        Self::Violet500,
        Self::Violet600,
        Self::Violet700,
        Self::Violet800,
        Self::Violet900,
        Self::Violet950,
        Self::Purple50,
        Self::Purple100,
        Self::Purple200,
        Self::Purple300,
        Self::Purple400,
        Self::Purple500,
        Self::Purple600,
        Self::Purple700,
        Self::Purple800,
        Self::Purple900,
        Self::Purple950,
        Self::Fuchsia50,
        Self::Fuchsia100,
        Self::Fuchsia200,
        Self::Fuchsia300,
        Self::Fuchsia400,
        Self::Fuchsia500,
        Self::Fuchsia600,
        Self::Fuchsia700,
        Self::Fuchsia800,
        Self::Fuchsia900,
        Self::Fuchsia950,
        Self::Pink50,
        Self::Pink100,
        Self::Pink200,
        Self::Pink300,
        Self::Pink400,
        Self::Pink500,
        Self::Pink600,
        Self::Pink700,
        Self::Pink800,
        Self::Pink900,
        Self::Pink950,
        Self::Rose50,
        Self::Rose100,
        Self::Rose200,
        Self::Rose300,
        Self::Rose400,
        Self::Rose500,
        Self::Rose600,
        Self::Rose700,
        Self::Rose800,
        Self::Rose900,
        Self::Rose950,
        Self::Slate50,
        Self::Slate100,
        Self::Slate200,
        Self::Slate300,
        Self::Slate400,
        Self::Slate500,
        Self::Slate600,
        Self::Slate700,
        Self::Slate800,
        Self::Slate900,
        Self::Slate950,
        Self::Gray50,
        Self::Gray100,
        Self::Gray200,
        Self::Gray300,
        Self::Gray400,
        Self::Gray500,
        Self::Gray600,
        Self::Gray700,
        Self::Gray800,
        Self::Gray900,
        Self::Gray950,
        Self::Zinc50,
        Self::Zinc100,
        Self::Zinc200,
        Self::Zinc300,
        Self::Zinc400,
        Self::Zinc500,
        Self::Zinc600,
        Self::Zinc700,
        Self::Zinc800,
        Self::Zinc900,
        Self::Zinc950,
        Self::Neutral50,
        Self::Neutral100,
        Self::Neutral200,
        Self::Neutral300,
        Self::Neutral400,
        Self::Neutral500,
        Self::Neutral600,
        Self::Neutral700,
        Self::Neutral800,
        Self::Neutral900,
        Self::Neutral950,
        Self::Stone50,
        Self::Stone100,
        Self::Stone200,
        Self::Stone300,
        Self::Stone400,
        Self::Stone500,
        Self::Stone600,
        Self::Stone700,
        Self::Stone800,
        Self::Stone900,
        Self::Stone950,
        Self::Mauve50,
        Self::Mauve100,
        Self::Mauve200,
        Self::Mauve300,
        Self::Mauve400,
        Self::Mauve500,
        Self::Mauve600,
        Self::Mauve700,
        Self::Mauve800,
        Self::Mauve900,
        Self::Mauve950,
        Self::Olive50,
        Self::Olive100,
        Self::Olive200,
        Self::Olive300,
        Self::Olive400,
        Self::Olive500,
        Self::Olive600,
        Self::Olive700,
        Self::Olive800,
        Self::Olive900,
        Self::Olive950,
        Self::Mist50,
        Self::Mist100,
        Self::Mist200,
        Self::Mist300,
        Self::Mist400,
        Self::Mist500,
        Self::Mist600,
        Self::Mist700,
        Self::Mist800,
        Self::Mist900,
        Self::Mist950,
        Self::Taupe50,
        Self::Taupe100,
        Self::Taupe200,
        Self::Taupe300,
        Self::Taupe400,
        Self::Taupe500,
        Self::Taupe600,
        Self::Taupe700,
        Self::Taupe800,
        Self::Taupe900,
        Self::Taupe950,
    ];

    /// The palette entry's perceptual coordinates; `None` for transparent and explicit values.
    pub(crate) const fn palette_oklch(self) -> Option<Oklch> {
        match self {
            Self::Black => Some(Oklch::new(0.0, 0.0, 0.0)),
            Self::White => Some(Oklch::new(1.0, 0.0, 0.0)),
            Self::Transparent => None,
            Self::Red50 => Some(Oklch::new(0.971, 0.013, 17.38)),
            Self::Red100 => Some(Oklch::new(0.936, 0.032, 17.717)),
            Self::Red200 => Some(Oklch::new(0.885, 0.062, 18.334)),
            Self::Red300 => Some(Oklch::new(0.808, 0.114, 19.571)),
            Self::Red400 => Some(Oklch::new(0.704, 0.191, 22.216)),
            Self::Red500 => Some(Oklch::new(0.637, 0.237, 25.331)),
            Self::Red600 => Some(Oklch::new(0.577, 0.245, 27.325)),
            Self::Red700 => Some(Oklch::new(0.505, 0.213, 27.518)),
            Self::Red800 => Some(Oklch::new(0.444, 0.177, 26.899)),
            Self::Red900 => Some(Oklch::new(0.396, 0.141, 25.723)),
            Self::Red950 => Some(Oklch::new(0.258, 0.092, 26.042)),
            Self::Orange50 => Some(Oklch::new(0.98, 0.016, 73.684)),
            Self::Orange100 => Some(Oklch::new(0.954, 0.038, 75.164)),
            Self::Orange200 => Some(Oklch::new(0.901, 0.076, 70.697)),
            Self::Orange300 => Some(Oklch::new(0.837, 0.128, 66.29)),
            Self::Orange400 => Some(Oklch::new(0.75, 0.183, 55.934)),
            Self::Orange500 => Some(Oklch::new(0.705, 0.213, 47.604)),
            Self::Orange600 => Some(Oklch::new(0.646, 0.222, 41.116)),
            Self::Orange700 => Some(Oklch::new(0.553, 0.195, 38.402)),
            Self::Orange800 => Some(Oklch::new(0.47, 0.157, 37.304)),
            Self::Orange900 => Some(Oklch::new(0.408, 0.123, 38.172)),
            Self::Orange950 => Some(Oklch::new(0.266, 0.079, 36.259)),
            Self::Amber50 => Some(Oklch::new(0.987, 0.022, 95.277)),
            Self::Amber100 => Some(Oklch::new(0.962, 0.059, 95.617)),
            Self::Amber200 => Some(Oklch::new(0.924, 0.12, 95.746)),
            Self::Amber300 => Some(Oklch::new(0.879, 0.169, 91.605)),
            Self::Amber400 => Some(Oklch::new(0.828, 0.189, 84.429)),
            Self::Amber500 => Some(Oklch::new(0.769, 0.188, 70.08)),
            Self::Amber600 => Some(Oklch::new(0.666, 0.179, 58.318)),
            Self::Amber700 => Some(Oklch::new(0.555, 0.163, 48.998)),
            Self::Amber800 => Some(Oklch::new(0.473, 0.137, 46.201)),
            Self::Amber900 => Some(Oklch::new(0.414, 0.112, 45.904)),
            Self::Amber950 => Some(Oklch::new(0.279, 0.077, 45.635)),
            Self::Yellow50 => Some(Oklch::new(0.987, 0.026, 102.212)),
            Self::Yellow100 => Some(Oklch::new(0.973, 0.071, 103.193)),
            Self::Yellow200 => Some(Oklch::new(0.945, 0.129, 101.54)),
            Self::Yellow300 => Some(Oklch::new(0.905, 0.182, 98.111)),
            Self::Yellow400 => Some(Oklch::new(0.852, 0.199, 91.936)),
            Self::Yellow500 => Some(Oklch::new(0.795, 0.184, 86.047)),
            Self::Yellow600 => Some(Oklch::new(0.681, 0.162, 75.834)),
            Self::Yellow700 => Some(Oklch::new(0.554, 0.135, 66.442)),
            Self::Yellow800 => Some(Oklch::new(0.476, 0.114, 61.907)),
            Self::Yellow900 => Some(Oklch::new(0.421, 0.095, 57.708)),
            Self::Yellow950 => Some(Oklch::new(0.286, 0.066, 53.813)),
            Self::Lime50 => Some(Oklch::new(0.986, 0.031, 120.757)),
            Self::Lime100 => Some(Oklch::new(0.967, 0.067, 122.328)),
            Self::Lime200 => Some(Oklch::new(0.938, 0.127, 124.321)),
            Self::Lime300 => Some(Oklch::new(0.897, 0.196, 126.665)),
            Self::Lime400 => Some(Oklch::new(0.841, 0.238, 128.85)),
            Self::Lime500 => Some(Oklch::new(0.768, 0.233, 130.85)),
            Self::Lime600 => Some(Oklch::new(0.648, 0.2, 131.684)),
            Self::Lime700 => Some(Oklch::new(0.532, 0.157, 131.589)),
            Self::Lime800 => Some(Oklch::new(0.453, 0.124, 130.933)),
            Self::Lime900 => Some(Oklch::new(0.405, 0.101, 131.063)),
            Self::Lime950 => Some(Oklch::new(0.274, 0.072, 132.109)),
            Self::Green50 => Some(Oklch::new(0.982, 0.018, 155.826)),
            Self::Green100 => Some(Oklch::new(0.962, 0.044, 156.743)),
            Self::Green200 => Some(Oklch::new(0.925, 0.084, 155.995)),
            Self::Green300 => Some(Oklch::new(0.871, 0.15, 154.449)),
            Self::Green400 => Some(Oklch::new(0.792, 0.209, 151.711)),
            Self::Green500 => Some(Oklch::new(0.723, 0.219, 149.579)),
            Self::Green600 => Some(Oklch::new(0.627, 0.194, 149.214)),
            Self::Green700 => Some(Oklch::new(0.527, 0.154, 150.069)),
            Self::Green800 => Some(Oklch::new(0.448, 0.119, 151.328)),
            Self::Green900 => Some(Oklch::new(0.393, 0.095, 152.535)),
            Self::Green950 => Some(Oklch::new(0.266, 0.065, 152.934)),
            Self::Emerald50 => Some(Oklch::new(0.979, 0.021, 166.113)),
            Self::Emerald100 => Some(Oklch::new(0.95, 0.052, 163.051)),
            Self::Emerald200 => Some(Oklch::new(0.905, 0.093, 164.15)),
            Self::Emerald300 => Some(Oklch::new(0.845, 0.143, 164.978)),
            Self::Emerald400 => Some(Oklch::new(0.765, 0.177, 163.223)),
            Self::Emerald500 => Some(Oklch::new(0.696, 0.17, 162.48)),
            Self::Emerald600 => Some(Oklch::new(0.596, 0.145, 163.225)),
            Self::Emerald700 => Some(Oklch::new(0.508, 0.118, 165.612)),
            Self::Emerald800 => Some(Oklch::new(0.432, 0.095, 166.913)),
            Self::Emerald900 => Some(Oklch::new(0.378, 0.077, 168.94)),
            Self::Emerald950 => Some(Oklch::new(0.262, 0.051, 172.552)),
            Self::Teal50 => Some(Oklch::new(0.984, 0.014, 180.72)),
            Self::Teal100 => Some(Oklch::new(0.953, 0.051, 180.801)),
            Self::Teal200 => Some(Oklch::new(0.91, 0.096, 180.426)),
            Self::Teal300 => Some(Oklch::new(0.855, 0.138, 181.071)),
            Self::Teal400 => Some(Oklch::new(0.777, 0.152, 181.912)),
            Self::Teal500 => Some(Oklch::new(0.704, 0.14, 182.503)),
            Self::Teal600 => Some(Oklch::new(0.6, 0.118, 184.704)),
            Self::Teal700 => Some(Oklch::new(0.511, 0.096, 186.391)),
            Self::Teal800 => Some(Oklch::new(0.437, 0.078, 188.216)),
            Self::Teal900 => Some(Oklch::new(0.386, 0.063, 188.416)),
            Self::Teal950 => Some(Oklch::new(0.277, 0.046, 192.524)),
            Self::Cyan50 => Some(Oklch::new(0.984, 0.019, 200.873)),
            Self::Cyan100 => Some(Oklch::new(0.956, 0.045, 203.388)),
            Self::Cyan200 => Some(Oklch::new(0.917, 0.08, 205.041)),
            Self::Cyan300 => Some(Oklch::new(0.865, 0.127, 207.078)),
            Self::Cyan400 => Some(Oklch::new(0.789, 0.154, 211.53)),
            Self::Cyan500 => Some(Oklch::new(0.715, 0.143, 215.221)),
            Self::Cyan600 => Some(Oklch::new(0.609, 0.126, 221.723)),
            Self::Cyan700 => Some(Oklch::new(0.52, 0.105, 223.128)),
            Self::Cyan800 => Some(Oklch::new(0.45, 0.085, 224.283)),
            Self::Cyan900 => Some(Oklch::new(0.398, 0.07, 227.392)),
            Self::Cyan950 => Some(Oklch::new(0.302, 0.056, 229.695)),
            Self::Sky50 => Some(Oklch::new(0.977, 0.013, 236.62)),
            Self::Sky100 => Some(Oklch::new(0.951, 0.026, 236.824)),
            Self::Sky200 => Some(Oklch::new(0.901, 0.058, 230.902)),
            Self::Sky300 => Some(Oklch::new(0.828, 0.111, 230.318)),
            Self::Sky400 => Some(Oklch::new(0.746, 0.16, 232.661)),
            Self::Sky500 => Some(Oklch::new(0.685, 0.169, 237.323)),
            Self::Sky600 => Some(Oklch::new(0.588, 0.158, 241.966)),
            Self::Sky700 => Some(Oklch::new(0.5, 0.134, 242.749)),
            Self::Sky800 => Some(Oklch::new(0.443, 0.11, 240.79)),
            Self::Sky900 => Some(Oklch::new(0.391, 0.09, 240.876)),
            Self::Sky950 => Some(Oklch::new(0.293, 0.066, 243.157)),
            Self::Blue50 => Some(Oklch::new(0.97, 0.014, 254.604)),
            Self::Blue100 => Some(Oklch::new(0.932, 0.032, 255.585)),
            Self::Blue200 => Some(Oklch::new(0.882, 0.059, 254.128)),
            Self::Blue300 => Some(Oklch::new(0.809, 0.105, 251.813)),
            Self::Blue400 => Some(Oklch::new(0.707, 0.165, 254.624)),
            Self::Blue500 => Some(Oklch::new(0.623, 0.214, 259.815)),
            Self::Blue600 => Some(Oklch::new(0.546, 0.245, 262.881)),
            Self::Blue700 => Some(Oklch::new(0.488, 0.243, 264.376)),
            Self::Blue800 => Some(Oklch::new(0.424, 0.199, 265.638)),
            Self::Blue900 => Some(Oklch::new(0.379, 0.146, 265.522)),
            Self::Blue950 => Some(Oklch::new(0.282, 0.091, 267.935)),
            Self::Indigo50 => Some(Oklch::new(0.962, 0.018, 272.314)),
            Self::Indigo100 => Some(Oklch::new(0.93, 0.034, 272.788)),
            Self::Indigo200 => Some(Oklch::new(0.87, 0.065, 274.039)),
            Self::Indigo300 => Some(Oklch::new(0.785, 0.115, 274.713)),
            Self::Indigo400 => Some(Oklch::new(0.673, 0.182, 276.935)),
            Self::Indigo500 => Some(Oklch::new(0.585, 0.233, 277.117)),
            Self::Indigo600 => Some(Oklch::new(0.511, 0.262, 276.966)),
            Self::Indigo700 => Some(Oklch::new(0.457, 0.24, 277.023)),
            Self::Indigo800 => Some(Oklch::new(0.398, 0.195, 277.366)),
            Self::Indigo900 => Some(Oklch::new(0.359, 0.144, 278.697)),
            Self::Indigo950 => Some(Oklch::new(0.257, 0.09, 281.288)),
            Self::Violet50 => Some(Oklch::new(0.969, 0.016, 293.756)),
            Self::Violet100 => Some(Oklch::new(0.943, 0.029, 294.588)),
            Self::Violet200 => Some(Oklch::new(0.894, 0.057, 293.283)),
            Self::Violet300 => Some(Oklch::new(0.811, 0.111, 293.571)),
            Self::Violet400 => Some(Oklch::new(0.702, 0.183, 293.541)),
            Self::Violet500 => Some(Oklch::new(0.606, 0.25, 292.717)),
            Self::Violet600 => Some(Oklch::new(0.541, 0.281, 293.009)),
            Self::Violet700 => Some(Oklch::new(0.491, 0.27, 292.581)),
            Self::Violet800 => Some(Oklch::new(0.432, 0.232, 292.759)),
            Self::Violet900 => Some(Oklch::new(0.38, 0.189, 293.745)),
            Self::Violet950 => Some(Oklch::new(0.283, 0.141, 291.089)),
            Self::Purple50 => Some(Oklch::new(0.977, 0.014, 308.299)),
            Self::Purple100 => Some(Oklch::new(0.946, 0.033, 307.174)),
            Self::Purple200 => Some(Oklch::new(0.902, 0.063, 306.703)),
            Self::Purple300 => Some(Oklch::new(0.827, 0.119, 306.383)),
            Self::Purple400 => Some(Oklch::new(0.714, 0.203, 305.504)),
            Self::Purple500 => Some(Oklch::new(0.627, 0.265, 303.9)),
            Self::Purple600 => Some(Oklch::new(0.558, 0.288, 302.321)),
            Self::Purple700 => Some(Oklch::new(0.496, 0.265, 301.924)),
            Self::Purple800 => Some(Oklch::new(0.438, 0.218, 303.724)),
            Self::Purple900 => Some(Oklch::new(0.381, 0.176, 304.987)),
            Self::Purple950 => Some(Oklch::new(0.291, 0.149, 302.717)),
            Self::Fuchsia50 => Some(Oklch::new(0.977, 0.017, 320.058)),
            Self::Fuchsia100 => Some(Oklch::new(0.952, 0.037, 318.852)),
            Self::Fuchsia200 => Some(Oklch::new(0.903, 0.076, 319.62)),
            Self::Fuchsia300 => Some(Oklch::new(0.833, 0.145, 321.434)),
            Self::Fuchsia400 => Some(Oklch::new(0.74, 0.238, 322.16)),
            Self::Fuchsia500 => Some(Oklch::new(0.667, 0.295, 322.15)),
            Self::Fuchsia600 => Some(Oklch::new(0.591, 0.293, 322.896)),
            Self::Fuchsia700 => Some(Oklch::new(0.518, 0.253, 323.949)),
            Self::Fuchsia800 => Some(Oklch::new(0.452, 0.211, 324.591)),
            Self::Fuchsia900 => Some(Oklch::new(0.401, 0.17, 325.612)),
            Self::Fuchsia950 => Some(Oklch::new(0.293, 0.136, 325.661)),
            Self::Pink50 => Some(Oklch::new(0.971, 0.014, 343.198)),
            Self::Pink100 => Some(Oklch::new(0.948, 0.028, 342.258)),
            Self::Pink200 => Some(Oklch::new(0.899, 0.061, 343.231)),
            Self::Pink300 => Some(Oklch::new(0.823, 0.12, 346.018)),
            Self::Pink400 => Some(Oklch::new(0.718, 0.202, 349.761)),
            Self::Pink500 => Some(Oklch::new(0.656, 0.241, 354.308)),
            Self::Pink600 => Some(Oklch::new(0.592, 0.249, 0.584)),
            Self::Pink700 => Some(Oklch::new(0.525, 0.223, 3.958)),
            Self::Pink800 => Some(Oklch::new(0.459, 0.187, 3.815)),
            Self::Pink900 => Some(Oklch::new(0.408, 0.153, 2.432)),
            Self::Pink950 => Some(Oklch::new(0.284, 0.109, 3.907)),
            Self::Rose50 => Some(Oklch::new(0.969, 0.015, 12.422)),
            Self::Rose100 => Some(Oklch::new(0.941, 0.03, 12.58)),
            Self::Rose200 => Some(Oklch::new(0.892, 0.058, 10.001)),
            Self::Rose300 => Some(Oklch::new(0.81, 0.117, 11.638)),
            Self::Rose400 => Some(Oklch::new(0.712, 0.194, 13.428)),
            Self::Rose500 => Some(Oklch::new(0.645, 0.246, 16.439)),
            Self::Rose600 => Some(Oklch::new(0.586, 0.253, 17.585)),
            Self::Rose700 => Some(Oklch::new(0.514, 0.222, 16.935)),
            Self::Rose800 => Some(Oklch::new(0.455, 0.188, 13.697)),
            Self::Rose900 => Some(Oklch::new(0.41, 0.159, 10.272)),
            Self::Rose950 => Some(Oklch::new(0.271, 0.105, 12.094)),
            Self::Slate50 => Some(Oklch::new(0.984, 0.003, 247.858)),
            Self::Slate100 => Some(Oklch::new(0.968, 0.007, 247.896)),
            Self::Slate200 => Some(Oklch::new(0.929, 0.013, 255.508)),
            Self::Slate300 => Some(Oklch::new(0.869, 0.022, 252.894)),
            Self::Slate400 => Some(Oklch::new(0.704, 0.04, 256.788)),
            Self::Slate500 => Some(Oklch::new(0.554, 0.046, 257.417)),
            Self::Slate600 => Some(Oklch::new(0.446, 0.043, 257.281)),
            Self::Slate700 => Some(Oklch::new(0.372, 0.044, 257.287)),
            Self::Slate800 => Some(Oklch::new(0.279, 0.041, 260.031)),
            Self::Slate900 => Some(Oklch::new(0.208, 0.042, 265.755)),
            Self::Slate950 => Some(Oklch::new(0.129, 0.042, 264.695)),
            Self::Gray50 => Some(Oklch::new(0.985, 0.002, 247.839)),
            Self::Gray100 => Some(Oklch::new(0.967, 0.003, 264.542)),
            Self::Gray200 => Some(Oklch::new(0.928, 0.006, 264.531)),
            Self::Gray300 => Some(Oklch::new(0.872, 0.01, 258.338)),
            Self::Gray400 => Some(Oklch::new(0.707, 0.022, 261.325)),
            Self::Gray500 => Some(Oklch::new(0.551, 0.027, 264.364)),
            Self::Gray600 => Some(Oklch::new(0.446, 0.03, 256.802)),
            Self::Gray700 => Some(Oklch::new(0.373, 0.034, 259.733)),
            Self::Gray800 => Some(Oklch::new(0.278, 0.033, 256.848)),
            Self::Gray900 => Some(Oklch::new(0.21, 0.034, 264.665)),
            Self::Gray950 => Some(Oklch::new(0.13, 0.028, 261.692)),
            Self::Zinc50 => Some(Oklch::new(0.985, 0.0, 0.0)),
            Self::Zinc100 => Some(Oklch::new(0.967, 0.001, 286.375)),
            Self::Zinc200 => Some(Oklch::new(0.92, 0.004, 286.32)),
            Self::Zinc300 => Some(Oklch::new(0.871, 0.006, 286.286)),
            Self::Zinc400 => Some(Oklch::new(0.705, 0.015, 286.067)),
            Self::Zinc500 => Some(Oklch::new(0.552, 0.016, 285.938)),
            Self::Zinc600 => Some(Oklch::new(0.442, 0.017, 285.786)),
            Self::Zinc700 => Some(Oklch::new(0.37, 0.013, 285.805)),
            Self::Zinc800 => Some(Oklch::new(0.274, 0.006, 286.033)),
            Self::Zinc900 => Some(Oklch::new(0.21, 0.006, 285.885)),
            Self::Zinc950 => Some(Oklch::new(0.141, 0.005, 285.823)),
            Self::Neutral50 => Some(Oklch::new(0.985, 0.0, 0.0)),
            Self::Neutral100 => Some(Oklch::new(0.97, 0.0, 0.0)),
            Self::Neutral200 => Some(Oklch::new(0.922, 0.0, 0.0)),
            Self::Neutral300 => Some(Oklch::new(0.87, 0.0, 0.0)),
            Self::Neutral400 => Some(Oklch::new(0.708, 0.0, 0.0)),
            Self::Neutral500 => Some(Oklch::new(0.556, 0.0, 0.0)),
            Self::Neutral600 => Some(Oklch::new(0.439, 0.0, 0.0)),
            Self::Neutral700 => Some(Oklch::new(0.371, 0.0, 0.0)),
            Self::Neutral800 => Some(Oklch::new(0.269, 0.0, 0.0)),
            Self::Neutral900 => Some(Oklch::new(0.205, 0.0, 0.0)),
            Self::Neutral950 => Some(Oklch::new(0.145, 0.0, 0.0)),
            Self::Stone50 => Some(Oklch::new(0.985, 0.001, 106.423)),
            Self::Stone100 => Some(Oklch::new(0.97, 0.001, 106.424)),
            Self::Stone200 => Some(Oklch::new(0.923, 0.003, 48.717)),
            Self::Stone300 => Some(Oklch::new(0.869, 0.005, 56.366)),
            Self::Stone400 => Some(Oklch::new(0.709, 0.01, 56.259)),
            Self::Stone500 => Some(Oklch::new(0.553, 0.013, 58.071)),
            Self::Stone600 => Some(Oklch::new(0.444, 0.011, 73.639)),
            Self::Stone700 => Some(Oklch::new(0.374, 0.01, 67.558)),
            Self::Stone800 => Some(Oklch::new(0.268, 0.007, 34.298)),
            Self::Stone900 => Some(Oklch::new(0.216, 0.006, 56.043)),
            Self::Stone950 => Some(Oklch::new(0.147, 0.004, 49.25)),
            Self::Mauve50 => Some(Oklch::new(0.985, 0.0, 0.0)),
            Self::Mauve100 => Some(Oklch::new(0.96, 0.003, 325.6)),
            Self::Mauve200 => Some(Oklch::new(0.922, 0.005, 325.62)),
            Self::Mauve300 => Some(Oklch::new(0.865, 0.012, 325.68)),
            Self::Mauve400 => Some(Oklch::new(0.711, 0.019, 323.02)),
            Self::Mauve500 => Some(Oklch::new(0.542, 0.034, 322.5)),
            Self::Mauve600 => Some(Oklch::new(0.435, 0.029, 321.78)),
            Self::Mauve700 => Some(Oklch::new(0.364, 0.029, 323.89)),
            Self::Mauve800 => Some(Oklch::new(0.263, 0.024, 320.12)),
            Self::Mauve900 => Some(Oklch::new(0.212, 0.019, 322.12)),
            Self::Mauve950 => Some(Oklch::new(0.145, 0.008, 326.0)),
            Self::Olive50 => Some(Oklch::new(0.988, 0.003, 106.5)),
            Self::Olive100 => Some(Oklch::new(0.966, 0.005, 106.5)),
            Self::Olive200 => Some(Oklch::new(0.93, 0.007, 106.5)),
            Self::Olive300 => Some(Oklch::new(0.88, 0.011, 106.6)),
            Self::Olive400 => Some(Oklch::new(0.737, 0.021, 106.9)),
            Self::Olive500 => Some(Oklch::new(0.58, 0.031, 107.3)),
            Self::Olive600 => Some(Oklch::new(0.466, 0.025, 107.3)),
            Self::Olive700 => Some(Oklch::new(0.394, 0.023, 107.4)),
            Self::Olive800 => Some(Oklch::new(0.286, 0.016, 107.4)),
            Self::Olive900 => Some(Oklch::new(0.228, 0.013, 107.4)),
            Self::Olive950 => Some(Oklch::new(0.153, 0.006, 107.1)),
            Self::Mist50 => Some(Oklch::new(0.987, 0.002, 197.1)),
            Self::Mist100 => Some(Oklch::new(0.963, 0.002, 197.1)),
            Self::Mist200 => Some(Oklch::new(0.925, 0.005, 214.3)),
            Self::Mist300 => Some(Oklch::new(0.872, 0.007, 219.6)),
            Self::Mist400 => Some(Oklch::new(0.723, 0.014, 214.4)),
            Self::Mist500 => Some(Oklch::new(0.56, 0.021, 213.5)),
            Self::Mist600 => Some(Oklch::new(0.45, 0.017, 213.2)),
            Self::Mist700 => Some(Oklch::new(0.378, 0.015, 216.0)),
            Self::Mist800 => Some(Oklch::new(0.275, 0.011, 216.9)),
            Self::Mist900 => Some(Oklch::new(0.218, 0.008, 223.9)),
            Self::Mist950 => Some(Oklch::new(0.148, 0.004, 228.8)),
            Self::Taupe50 => Some(Oklch::new(0.986, 0.002, 67.8)),
            Self::Taupe100 => Some(Oklch::new(0.96, 0.002, 17.2)),
            Self::Taupe200 => Some(Oklch::new(0.922, 0.005, 34.3)),
            Self::Taupe300 => Some(Oklch::new(0.868, 0.007, 39.5)),
            Self::Taupe400 => Some(Oklch::new(0.714, 0.014, 41.2)),
            Self::Taupe500 => Some(Oklch::new(0.547, 0.021, 43.1)),
            Self::Taupe600 => Some(Oklch::new(0.438, 0.017, 39.3)),
            Self::Taupe700 => Some(Oklch::new(0.367, 0.016, 35.7)),
            Self::Taupe800 => Some(Oklch::new(0.268, 0.011, 36.5)),
            Self::Taupe900 => Some(Oklch::new(0.214, 0.009, 43.1)),
            Self::Taupe950 => Some(Oklch::new(0.147, 0.004, 49.3)),
            Self::Srgb(_) | Self::Role(..) => None,
        }
    }

    /// The palette name of the entry, such as `blue-500`; `None` for explicit values.
    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::Black => Some("black"),
            Self::White => Some("white"),
            Self::Transparent => Some("transparent"),
            Self::Red50 => Some("red-50"),
            Self::Red100 => Some("red-100"),
            Self::Red200 => Some("red-200"),
            Self::Red300 => Some("red-300"),
            Self::Red400 => Some("red-400"),
            Self::Red500 => Some("red-500"),
            Self::Red600 => Some("red-600"),
            Self::Red700 => Some("red-700"),
            Self::Red800 => Some("red-800"),
            Self::Red900 => Some("red-900"),
            Self::Red950 => Some("red-950"),
            Self::Orange50 => Some("orange-50"),
            Self::Orange100 => Some("orange-100"),
            Self::Orange200 => Some("orange-200"),
            Self::Orange300 => Some("orange-300"),
            Self::Orange400 => Some("orange-400"),
            Self::Orange500 => Some("orange-500"),
            Self::Orange600 => Some("orange-600"),
            Self::Orange700 => Some("orange-700"),
            Self::Orange800 => Some("orange-800"),
            Self::Orange900 => Some("orange-900"),
            Self::Orange950 => Some("orange-950"),
            Self::Amber50 => Some("amber-50"),
            Self::Amber100 => Some("amber-100"),
            Self::Amber200 => Some("amber-200"),
            Self::Amber300 => Some("amber-300"),
            Self::Amber400 => Some("amber-400"),
            Self::Amber500 => Some("amber-500"),
            Self::Amber600 => Some("amber-600"),
            Self::Amber700 => Some("amber-700"),
            Self::Amber800 => Some("amber-800"),
            Self::Amber900 => Some("amber-900"),
            Self::Amber950 => Some("amber-950"),
            Self::Yellow50 => Some("yellow-50"),
            Self::Yellow100 => Some("yellow-100"),
            Self::Yellow200 => Some("yellow-200"),
            Self::Yellow300 => Some("yellow-300"),
            Self::Yellow400 => Some("yellow-400"),
            Self::Yellow500 => Some("yellow-500"),
            Self::Yellow600 => Some("yellow-600"),
            Self::Yellow700 => Some("yellow-700"),
            Self::Yellow800 => Some("yellow-800"),
            Self::Yellow900 => Some("yellow-900"),
            Self::Yellow950 => Some("yellow-950"),
            Self::Lime50 => Some("lime-50"),
            Self::Lime100 => Some("lime-100"),
            Self::Lime200 => Some("lime-200"),
            Self::Lime300 => Some("lime-300"),
            Self::Lime400 => Some("lime-400"),
            Self::Lime500 => Some("lime-500"),
            Self::Lime600 => Some("lime-600"),
            Self::Lime700 => Some("lime-700"),
            Self::Lime800 => Some("lime-800"),
            Self::Lime900 => Some("lime-900"),
            Self::Lime950 => Some("lime-950"),
            Self::Green50 => Some("green-50"),
            Self::Green100 => Some("green-100"),
            Self::Green200 => Some("green-200"),
            Self::Green300 => Some("green-300"),
            Self::Green400 => Some("green-400"),
            Self::Green500 => Some("green-500"),
            Self::Green600 => Some("green-600"),
            Self::Green700 => Some("green-700"),
            Self::Green800 => Some("green-800"),
            Self::Green900 => Some("green-900"),
            Self::Green950 => Some("green-950"),
            Self::Emerald50 => Some("emerald-50"),
            Self::Emerald100 => Some("emerald-100"),
            Self::Emerald200 => Some("emerald-200"),
            Self::Emerald300 => Some("emerald-300"),
            Self::Emerald400 => Some("emerald-400"),
            Self::Emerald500 => Some("emerald-500"),
            Self::Emerald600 => Some("emerald-600"),
            Self::Emerald700 => Some("emerald-700"),
            Self::Emerald800 => Some("emerald-800"),
            Self::Emerald900 => Some("emerald-900"),
            Self::Emerald950 => Some("emerald-950"),
            Self::Teal50 => Some("teal-50"),
            Self::Teal100 => Some("teal-100"),
            Self::Teal200 => Some("teal-200"),
            Self::Teal300 => Some("teal-300"),
            Self::Teal400 => Some("teal-400"),
            Self::Teal500 => Some("teal-500"),
            Self::Teal600 => Some("teal-600"),
            Self::Teal700 => Some("teal-700"),
            Self::Teal800 => Some("teal-800"),
            Self::Teal900 => Some("teal-900"),
            Self::Teal950 => Some("teal-950"),
            Self::Cyan50 => Some("cyan-50"),
            Self::Cyan100 => Some("cyan-100"),
            Self::Cyan200 => Some("cyan-200"),
            Self::Cyan300 => Some("cyan-300"),
            Self::Cyan400 => Some("cyan-400"),
            Self::Cyan500 => Some("cyan-500"),
            Self::Cyan600 => Some("cyan-600"),
            Self::Cyan700 => Some("cyan-700"),
            Self::Cyan800 => Some("cyan-800"),
            Self::Cyan900 => Some("cyan-900"),
            Self::Cyan950 => Some("cyan-950"),
            Self::Sky50 => Some("sky-50"),
            Self::Sky100 => Some("sky-100"),
            Self::Sky200 => Some("sky-200"),
            Self::Sky300 => Some("sky-300"),
            Self::Sky400 => Some("sky-400"),
            Self::Sky500 => Some("sky-500"),
            Self::Sky600 => Some("sky-600"),
            Self::Sky700 => Some("sky-700"),
            Self::Sky800 => Some("sky-800"),
            Self::Sky900 => Some("sky-900"),
            Self::Sky950 => Some("sky-950"),
            Self::Blue50 => Some("blue-50"),
            Self::Blue100 => Some("blue-100"),
            Self::Blue200 => Some("blue-200"),
            Self::Blue300 => Some("blue-300"),
            Self::Blue400 => Some("blue-400"),
            Self::Blue500 => Some("blue-500"),
            Self::Blue600 => Some("blue-600"),
            Self::Blue700 => Some("blue-700"),
            Self::Blue800 => Some("blue-800"),
            Self::Blue900 => Some("blue-900"),
            Self::Blue950 => Some("blue-950"),
            Self::Indigo50 => Some("indigo-50"),
            Self::Indigo100 => Some("indigo-100"),
            Self::Indigo200 => Some("indigo-200"),
            Self::Indigo300 => Some("indigo-300"),
            Self::Indigo400 => Some("indigo-400"),
            Self::Indigo500 => Some("indigo-500"),
            Self::Indigo600 => Some("indigo-600"),
            Self::Indigo700 => Some("indigo-700"),
            Self::Indigo800 => Some("indigo-800"),
            Self::Indigo900 => Some("indigo-900"),
            Self::Indigo950 => Some("indigo-950"),
            Self::Violet50 => Some("violet-50"),
            Self::Violet100 => Some("violet-100"),
            Self::Violet200 => Some("violet-200"),
            Self::Violet300 => Some("violet-300"),
            Self::Violet400 => Some("violet-400"),
            Self::Violet500 => Some("violet-500"),
            Self::Violet600 => Some("violet-600"),
            Self::Violet700 => Some("violet-700"),
            Self::Violet800 => Some("violet-800"),
            Self::Violet900 => Some("violet-900"),
            Self::Violet950 => Some("violet-950"),
            Self::Purple50 => Some("purple-50"),
            Self::Purple100 => Some("purple-100"),
            Self::Purple200 => Some("purple-200"),
            Self::Purple300 => Some("purple-300"),
            Self::Purple400 => Some("purple-400"),
            Self::Purple500 => Some("purple-500"),
            Self::Purple600 => Some("purple-600"),
            Self::Purple700 => Some("purple-700"),
            Self::Purple800 => Some("purple-800"),
            Self::Purple900 => Some("purple-900"),
            Self::Purple950 => Some("purple-950"),
            Self::Fuchsia50 => Some("fuchsia-50"),
            Self::Fuchsia100 => Some("fuchsia-100"),
            Self::Fuchsia200 => Some("fuchsia-200"),
            Self::Fuchsia300 => Some("fuchsia-300"),
            Self::Fuchsia400 => Some("fuchsia-400"),
            Self::Fuchsia500 => Some("fuchsia-500"),
            Self::Fuchsia600 => Some("fuchsia-600"),
            Self::Fuchsia700 => Some("fuchsia-700"),
            Self::Fuchsia800 => Some("fuchsia-800"),
            Self::Fuchsia900 => Some("fuchsia-900"),
            Self::Fuchsia950 => Some("fuchsia-950"),
            Self::Pink50 => Some("pink-50"),
            Self::Pink100 => Some("pink-100"),
            Self::Pink200 => Some("pink-200"),
            Self::Pink300 => Some("pink-300"),
            Self::Pink400 => Some("pink-400"),
            Self::Pink500 => Some("pink-500"),
            Self::Pink600 => Some("pink-600"),
            Self::Pink700 => Some("pink-700"),
            Self::Pink800 => Some("pink-800"),
            Self::Pink900 => Some("pink-900"),
            Self::Pink950 => Some("pink-950"),
            Self::Rose50 => Some("rose-50"),
            Self::Rose100 => Some("rose-100"),
            Self::Rose200 => Some("rose-200"),
            Self::Rose300 => Some("rose-300"),
            Self::Rose400 => Some("rose-400"),
            Self::Rose500 => Some("rose-500"),
            Self::Rose600 => Some("rose-600"),
            Self::Rose700 => Some("rose-700"),
            Self::Rose800 => Some("rose-800"),
            Self::Rose900 => Some("rose-900"),
            Self::Rose950 => Some("rose-950"),
            Self::Slate50 => Some("slate-50"),
            Self::Slate100 => Some("slate-100"),
            Self::Slate200 => Some("slate-200"),
            Self::Slate300 => Some("slate-300"),
            Self::Slate400 => Some("slate-400"),
            Self::Slate500 => Some("slate-500"),
            Self::Slate600 => Some("slate-600"),
            Self::Slate700 => Some("slate-700"),
            Self::Slate800 => Some("slate-800"),
            Self::Slate900 => Some("slate-900"),
            Self::Slate950 => Some("slate-950"),
            Self::Gray50 => Some("gray-50"),
            Self::Gray100 => Some("gray-100"),
            Self::Gray200 => Some("gray-200"),
            Self::Gray300 => Some("gray-300"),
            Self::Gray400 => Some("gray-400"),
            Self::Gray500 => Some("gray-500"),
            Self::Gray600 => Some("gray-600"),
            Self::Gray700 => Some("gray-700"),
            Self::Gray800 => Some("gray-800"),
            Self::Gray900 => Some("gray-900"),
            Self::Gray950 => Some("gray-950"),
            Self::Zinc50 => Some("zinc-50"),
            Self::Zinc100 => Some("zinc-100"),
            Self::Zinc200 => Some("zinc-200"),
            Self::Zinc300 => Some("zinc-300"),
            Self::Zinc400 => Some("zinc-400"),
            Self::Zinc500 => Some("zinc-500"),
            Self::Zinc600 => Some("zinc-600"),
            Self::Zinc700 => Some("zinc-700"),
            Self::Zinc800 => Some("zinc-800"),
            Self::Zinc900 => Some("zinc-900"),
            Self::Zinc950 => Some("zinc-950"),
            Self::Neutral50 => Some("neutral-50"),
            Self::Neutral100 => Some("neutral-100"),
            Self::Neutral200 => Some("neutral-200"),
            Self::Neutral300 => Some("neutral-300"),
            Self::Neutral400 => Some("neutral-400"),
            Self::Neutral500 => Some("neutral-500"),
            Self::Neutral600 => Some("neutral-600"),
            Self::Neutral700 => Some("neutral-700"),
            Self::Neutral800 => Some("neutral-800"),
            Self::Neutral900 => Some("neutral-900"),
            Self::Neutral950 => Some("neutral-950"),
            Self::Stone50 => Some("stone-50"),
            Self::Stone100 => Some("stone-100"),
            Self::Stone200 => Some("stone-200"),
            Self::Stone300 => Some("stone-300"),
            Self::Stone400 => Some("stone-400"),
            Self::Stone500 => Some("stone-500"),
            Self::Stone600 => Some("stone-600"),
            Self::Stone700 => Some("stone-700"),
            Self::Stone800 => Some("stone-800"),
            Self::Stone900 => Some("stone-900"),
            Self::Stone950 => Some("stone-950"),
            Self::Mauve50 => Some("mauve-50"),
            Self::Mauve100 => Some("mauve-100"),
            Self::Mauve200 => Some("mauve-200"),
            Self::Mauve300 => Some("mauve-300"),
            Self::Mauve400 => Some("mauve-400"),
            Self::Mauve500 => Some("mauve-500"),
            Self::Mauve600 => Some("mauve-600"),
            Self::Mauve700 => Some("mauve-700"),
            Self::Mauve800 => Some("mauve-800"),
            Self::Mauve900 => Some("mauve-900"),
            Self::Mauve950 => Some("mauve-950"),
            Self::Olive50 => Some("olive-50"),
            Self::Olive100 => Some("olive-100"),
            Self::Olive200 => Some("olive-200"),
            Self::Olive300 => Some("olive-300"),
            Self::Olive400 => Some("olive-400"),
            Self::Olive500 => Some("olive-500"),
            Self::Olive600 => Some("olive-600"),
            Self::Olive700 => Some("olive-700"),
            Self::Olive800 => Some("olive-800"),
            Self::Olive900 => Some("olive-900"),
            Self::Olive950 => Some("olive-950"),
            Self::Mist50 => Some("mist-50"),
            Self::Mist100 => Some("mist-100"),
            Self::Mist200 => Some("mist-200"),
            Self::Mist300 => Some("mist-300"),
            Self::Mist400 => Some("mist-400"),
            Self::Mist500 => Some("mist-500"),
            Self::Mist600 => Some("mist-600"),
            Self::Mist700 => Some("mist-700"),
            Self::Mist800 => Some("mist-800"),
            Self::Mist900 => Some("mist-900"),
            Self::Mist950 => Some("mist-950"),
            Self::Taupe50 => Some("taupe-50"),
            Self::Taupe100 => Some("taupe-100"),
            Self::Taupe200 => Some("taupe-200"),
            Self::Taupe300 => Some("taupe-300"),
            Self::Taupe400 => Some("taupe-400"),
            Self::Taupe500 => Some("taupe-500"),
            Self::Taupe600 => Some("taupe-600"),
            Self::Taupe700 => Some("taupe-700"),
            Self::Taupe800 => Some("taupe-800"),
            Self::Taupe900 => Some("taupe-900"),
            Self::Taupe950 => Some("taupe-950"),
            Self::Srgb(_) | Self::Role(..) => None,
        }
    }
}
