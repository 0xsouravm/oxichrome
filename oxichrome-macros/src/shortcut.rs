pub const PLATFORMS: [&str; 5] = ["default", "mac", "windows", "linux", "chromeos"];

const KEYS: [&str; 49] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "Comma",
    "Period", "Home", "End", "PageUp", "PageDown", "Space", "Insert", "Delete", "Up", "Down",
    "Left", "Right",
];

const MEDIA_KEYS: [&str; 4] = ["MediaNextTrack", "MediaPlayPause", "MediaPrevTrack", "MediaStop"];

const MODIFIERS: [&str; 5] = ["Ctrl", "Alt", "Shift", "Command", "MacCtrl"];

const KEY_LIST: &str = "a letter `A`-`Z`, a digit `0`-`9`, `Comma`, `Period`, `Home`, `End`, `PageUp`, `PageDown`, `Space`, `Insert`, `Delete`, `Up`, `Down`, `Left` or `Right`";

include!("reserved.rs");

fn modifier_hint(token: &str, platform: &str) -> String {
    let lower = token.to_ascii_lowercase();
    let mac = platform == "mac";
    match lower.as_str() {
        "cmd" | "meta" | "super" | "win" | "windows" | "os" | "command" if !mac => format!(
            "`{token}` isn't a modifier here; in `{platform}` use `Ctrl` or `Alt` (on a Mac, `Ctrl` already means Command)"
        ),
        "cmd" | "meta" | "super" => format!("`{token}` isn't a modifier; write `Command`"),
        "control" | "ctl" => format!("`{token}` isn't a modifier; write `Ctrl`"),
        "option" | "opt" | "⌥" => format!(
            "`{token}` isn't a modifier both browsers accept (Firefox rejects `Option`); write `Alt`"
        ),
        "macctrl" if !mac => format!(
            "`MacCtrl` only works in `mac = \"...\"`; Chrome rejects the extension if it appears in `{platform}`"
        ),
        "search" => "`Search` only exists on ChromeOS and Firefox rejects it".to_string(),
        "altgr" => "`AltGr` isn't a modifier; use `Ctrl` or `Alt`".to_string(),
        _ => match MODIFIERS.iter().find(|name| name.eq_ignore_ascii_case(token)) {
            Some(name) => format!("modifiers are case sensitive: write `{name}`"),
            None => format!(
                "`{token}` isn't a modifier; use {}",
                if mac {
                    "`Ctrl` (Command), `Command`, `MacCtrl` (the Control key), `Alt` or `Shift`"
                } else {
                    "`Ctrl`, `Alt` or `Shift`"
                }
            ),
        },
    }
}

fn key_hint(token: &str) -> String {
    if let Some(name) = KEYS
        .iter()
        .chain(MEDIA_KEYS.iter())
        .find(|name| name.eq_ignore_ascii_case(token))
    {
        return format!("keys are case sensitive: write `{name}`");
    }
    let lower = token.to_ascii_lowercase();
    if lower.len() >= 2
        && lower.starts_with('f')
        && lower[1..].chars().all(|c| c.is_ascii_digit())
    {
        return format!("`{token}`: Firefox accepts function keys but Chrome rejects the whole extension; use {KEY_LIST}");
    }
    let named = match lower.as_str() {
        "," => Some("Comma"),
        "." => Some("Period"),
        " " | "spacebar" => Some("Space"),
        "del" => Some("Delete"),
        "ins" => Some("Insert"),
        "pgup" => Some("PageUp"),
        "pgdn" | "pgdown" => Some("PageDown"),
        "arrowup" | "↑" => Some("Up"),
        "arrowdown" | "↓" => Some("Down"),
        "arrowleft" | "←" => Some("Left"),
        "arrowright" | "→" => Some("Right"),
        "mediaprevioustrack" => Some("MediaPrevTrack"),
        _ => None,
    };
    if let Some(name) = named {
        return format!("write `{token}` as `{name}`");
    }
    format!("`{token}` isn't a key both browsers accept; use {KEY_LIST}, or a media key on its own")
}

pub fn effective_mac(text: &str) -> String {
    text.split('+')
        .map(|token| if token == "Ctrl" { "Command" } else { token })
        .collect::<Vec<_>>()
        .join("+")
}

pub fn check(text: &str, platform: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("a shortcut can't be empty; leave `shortcut` out for a command people assign themselves".to_string());
    }
    if text.chars().any(char::is_whitespace) {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        return Err(format!("write the shortcut without spaces: `{compact}`"));
    }
    let tokens: Vec<&str> = text.split('+').collect();
    if tokens.iter().any(|token| token.is_empty()) {
        return Err(format!(
            "`{text}` has an empty part; join the keys with single `+` signs, like `Alt+Shift+K`"
        ));
    }
    let (key, modifiers) = tokens.split_last().expect("split always yields a part");

    if MEDIA_KEYS.contains(key) {
        if !modifiers.is_empty() {
            return Err(format!(
                "media keys can't have modifiers in Chrome; write just `{key}`"
            ));
        }
        return Ok(());
    }
    if MODIFIERS.iter().any(|name| name.eq_ignore_ascii_case(key)) {
        return Err(format!(
            "`{text}` has no key; a shortcut ends with a key, like `Alt+Shift+K`"
        ));
    }
    if !KEYS.contains(key) {
        return Err(key_hint(key));
    }

    let mac = platform == "mac";
    for modifier in modifiers {
        let allowed = match *modifier {
            "Ctrl" | "Alt" | "Shift" => true,
            "Command" | "MacCtrl" => mac,
            _ => false,
        };
        if !allowed {
            return Err(modifier_hint(modifier, platform));
        }
    }
    for (index, modifier) in modifiers.iter().enumerate() {
        if modifiers[..index].contains(modifier) {
            return Err(format!("`{modifier}` appears twice in `{text}`"));
        }
    }

    let primaries: Vec<&str> = modifiers.iter().copied().filter(|m| *m != "Shift").collect();
    let shift = modifiers.contains(&"Shift");
    match primaries.as_slice() {
        [] if mac => {
            return Err(format!(
                "`{text}` needs `Ctrl`, `Command`, `MacCtrl` or `Alt`; both browsers reject a key with only `Shift` or nothing"
            ))
        }
        [] => {
            return Err(format!(
                "`{text}` needs `Ctrl` or `Alt`; both browsers reject a key with only `Shift` or nothing"
            ))
        }
        [_] => {}
        _ if primaries.contains(&"Alt") => {
            return Err(format!(
                "Chrome rejects `{text}` because Ctrl+Alt style shortcuts clash with AltGr; use one of {} plus `Shift` at most",
                primaries.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(" or ")
            ))
        }
        _ => {
            return Err(format!(
                "`{text}` combines {}; use one of them plus `Shift` at most",
                primaries.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(" and ")
            ))
        }
    }

    let canonical = if shift {
        format!("{}+Shift+{key}", primaries[0])
    } else {
        format!("{}+{key}", primaries[0])
    };
    if canonical != text {
        return Err(format!("write the modifier first and `Shift` second: `{canonical}`"));
    }

    let as_ctrl = if mac { effective_mac(text).replacen("Command+", "Ctrl+", 1) } else { text.to_string() };
    if let Some(reason) = reserved(&as_ctrl) {
        return Err(format!(
            "`{text}` belongs to the browser: {reason}, so this command would never run; pick another combination"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str, platform: &str) -> String {
        check(text, platform).unwrap_err()
    }

    #[test]
    fn accepts_shortcuts_both_browsers_take() {
        for text in [
            "Alt+Shift+K",
            "Ctrl+Shift+Y",
            "Alt+K",
            "Ctrl+Shift+Comma",
            "Alt+Shift+Up",
            "Alt+Shift+5",
            "MediaPlayPause",
        ] {
            assert_eq!(check(text, "default"), Ok(()), "{text}");
        }
        for text in ["Command+Shift+Y", "MacCtrl+Shift+K", "Alt+Shift+K"] {
            assert_eq!(check(text, "mac"), Ok(()), "{text}");
        }
    }

    #[test]
    fn rejects_what_a_browser_rejects() {
        assert!(err("K", "default").contains("needs `Ctrl` or `Alt`"));
        assert!(err("Shift+K", "default").contains("needs `Ctrl` or `Alt`"));
        assert!(err("Ctrl+Alt+K", "default").contains("AltGr"));
        assert!(err("Ctrl+Shift+Alt+K", "default").contains("AltGr"));
        assert!(err("Command+Alt+K", "mac").contains("AltGr"));
        assert!(err("Command+MacCtrl+K", "mac").contains("combines"));
        assert!(err("Ctrl+F5", "default").contains("function keys"));
        assert!(err("Ctrl+MediaPlayPause", "default").contains("media keys"));
        assert!(err("Ctrl+Shift+k", "default").contains("write `K`"));
        assert!(err("ctrl+Shift+K", "default").contains("write `Ctrl`"));
        assert!(err("Ctrl+Shift+Plus", "default").contains("isn't a key"));
        assert!(err("Ctrl+Shift", "default").contains("has no key"));
        assert!(err("", "default").contains("can't be empty"));
        assert!(err("Ctrl + Shift + K", "default").contains("`Ctrl+Shift+K`"));
        assert!(err("Ctrl++K", "default").contains("empty part"));
        assert!(err("Shift+Ctrl+K", "default").contains("`Ctrl+Shift+K`"));
        assert!(err("Ctrl+Ctrl+K", "default").contains("twice"));
        assert!(err("Command+Shift+K", "default").contains("Ctrl` already means Command"));
        assert!(err("MacCtrl+Shift+K", "windows").contains("only works in `mac"));
        assert!(err("Option+Shift+K", "mac").contains("Firefox rejects `Option`"));
        assert!(err("Cmd+Shift+K", "mac").contains("write `Command`"));
        assert!(err("Ctrl+Shift+ArrowUp", "default").contains("as `Up`"));
        assert!(err("Ctrl+T", "default").contains("belongs to the browser"));
        assert!(err("Command+T", "mac").contains("belongs to the browser"));
        assert!(err("Ctrl+T", "mac").contains("belongs to the browser"));
        assert_eq!(check("MacCtrl+T", "mac"), Ok(()));
        assert_eq!(check("Ctrl+Shift+U", "default"), Ok(()));
    }
}
