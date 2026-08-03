/// A renderer-independent non-premultiplied sRGBA colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}
impl Color {
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
    pub const fn rgb(red: f32, green: f32, blue: f32) -> Self {
        Self::new(red, green, blue, 1.0)
    }
    pub const fn red(self) -> f32 {
        self.red
    }
    pub const fn green(self) -> f32 {
        self.green
    }
    pub const fn blue(self) -> f32 {
        self.blue
    }
    pub const fn alpha(self) -> f32 {
        self.alpha
    }
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }
    pub fn clamped(self) -> Self {
        Self::new(
            self.red.clamp(0.0, 1.0),
            self.green.clamp(0.0, 1.0),
            self.blue.clamp(0.0, 1.0),
            self.alpha.clamp(0.0, 1.0),
        )
    }
}
pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
pub const RED: Color = Color::rgb(0.8, 0.1, 0.1);
pub const GREEN: Color = Color::rgb(0.1, 0.55, 0.2);
pub const BLUE: Color = Color::rgb(0.1, 0.3, 0.85);
/// Visual-only overlay handling for coincident surface features.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SurfaceOverlayMode {
    #[default]
    DepthBias,
    NormalOffset {
        distance: f32,
    },
    SeparatePass,
}
