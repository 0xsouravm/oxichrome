#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.trim().strip_prefix('#').filter(|hex| hex.is_ascii())?;
        let full = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 => hex.to_string(),
            _ => return None,
        };
        let channel = |at: usize| u8::from_str_radix(&full[at..at + 2], 16).ok();
        Some(Color { r: channel(0)?, g: channel(2)?, b: channel(4)? })
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn rgb(&self) -> String {
        format!("rgb({} {} {})", self.r, self.g, self.b)
    }

    pub fn hsl(&self) -> String {
        let (h, s, l) = self.to_hsl();
        format!("hsl({h} {s}% {l}%)")
    }

    fn to_hsl(self) -> (u32, u32, u32) {
        let [r, g, b] = [self.r, self.g, self.b].map(|c| f64::from(c) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let light = (max + min) / 2.0;
        let delta = max - min;
        if delta == 0.0 {
            return (0, 0, (light * 100.0).round() as u32);
        }
        let sat = delta / (1.0 - (2.0 * light - 1.0).abs());
        let hue = if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        (hue.round() as u32 % 360, (sat * 100.0).round() as u32, (light * 100.0).round() as u32)
    }

    fn luminance(&self) -> f64 {
        let linear = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    pub fn ink(&self) -> &'static str {
        if self.luminance() > 0.36 { "#111111" } else { "#ffffff" }
    }

    pub fn describe(&self) -> String {
        let (hue, sat, light) = self.to_hsl();
        let tone = match light {
            0..=8 => return "Black".into(),
            92.. => return "White".into(),
            9..=30 => "Dark ",
            70..=91 => "Light ",
            _ => "",
        };
        let name = match hue {
            _ if sat < 10 => "gray",
            0..=14 | 345..=359 => "red",
            15..=39 => "orange",
            40..=64 => "yellow",
            65..=159 => "green",
            160..=194 => "teal",
            195..=254 => "blue",
            255..=289 => "purple",
            _ => "pink",
        };
        let mut text = format!("{tone}{name}");
        if let Some(first) = text.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        text
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
        assert_eq!(Color::parse("#fff").unwrap().hex(), "#ffffff");
        assert_eq!(Color::parse("6e56cf"), None);
        assert_eq!(Color::parse("#12345"), None);
        assert_eq!(Color::parse("#gg0000"), None);
        assert_eq!(Color::parse("#éé"), None);
    }

    #[test]
    fn names_and_ink() {
        assert_eq!(Color::parse("#000").unwrap().describe(), "Black");
        assert_eq!(Color::parse("#808080").unwrap().describe(), "Gray");
        assert_eq!(Color::parse("#e5484d").unwrap().describe(), "Red");
        assert_eq!(Color::parse("#0f2350").unwrap().describe(), "Dark blue");
        assert_eq!(Color::parse("#1a3a8a").unwrap().describe(), "Blue");
        assert_eq!(Color::parse("#ffef5c").unwrap().ink(), "#111111");
        assert_eq!(Color::parse("#1a3a8a").unwrap().ink(), "#ffffff");
    }
}
