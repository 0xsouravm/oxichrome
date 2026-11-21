#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.trim().strip_prefix('#').filter(|hex| hex.len() == 6 && hex.is_ascii())?;
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
        Some(Color { r: channel(0)?, g: channel(2)?, b: channel(4)? })
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn rgb(&self) -> String {
        format!("rgb({} {} {})", self.r, self.g, self.b)
    }

    pub fn hsl(&self) -> String {
        let [r, g, b] = [self.r, self.g, self.b].map(|c| f64::from(c) / 255.0);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let (light, delta) = ((max + min) / 2.0, max - min);
        let (hue, sat) = if delta == 0.0 {
            (0.0, 0.0)
        } else {
            let hue = if max == r {
                ((g - b) / delta).rem_euclid(6.0)
            } else if max == g {
                (b - r) / delta + 2.0
            } else {
                (r - g) / delta + 4.0
            };
            (hue * 60.0, delta / (1.0 - (2.0 * light - 1.0).abs()))
        };
        let pct = |v: f64| (v * 100.0).round();
        format!("hsl({} {}% {}%)", hue.round() % 360.0, pct(sat), pct(light))
    }

    pub fn ink(&self) -> &'static str {
        let luma = 0.299 * f64::from(self.r) + 0.587 * f64::from(self.g) + 0.114 * f64::from(self.b);
        if luma > 150.0 { "#111111" } else { "#ffffff" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let c = Color::parse("#6E56CF").unwrap();
        assert_eq!(c.hex(), "#6e56cf");
        assert_eq!(c.rgb(), "rgb(110 86 207)");
        assert_eq!(c.hsl(), "hsl(252 56% 57%)");
        assert_eq!(Color::parse("#0090ff").unwrap().rgb(), "rgb(0 144 255)");
        assert_eq!(Color::parse("#808080").unwrap().hsl(), "hsl(0 0% 50%)");
        assert_eq!(Color::parse("6e56cf"), None);
        assert_eq!(Color::parse("#12345"), None);
        assert_eq!(Color::parse("#gg0000"), None);
        assert_eq!(Color::parse("#éééé"), None);
    }

    #[test]
    fn readable_ink() {
        assert_eq!(Color::parse("#ffef5c").unwrap().ink(), "#111111");
        assert_eq!(Color::parse("#1a3a8a").unwrap().ink(), "#ffffff");
    }
}
