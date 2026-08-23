//! Backend-neutral RGBA color token used by the document theme.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: u8::MAX,
        }
    }

    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn r(self) -> u8 {
        self.red
    }
    pub const fn g(self) -> u8 {
        self.green
    }
    pub const fn b(self) -> u8 {
        self.blue
    }
    pub const fn a(self) -> u8 {
        self.alpha
    }
}
