use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitBool, LitInt, LitStr, Token};

pub use crate::i18n::Text;

/// See <https://developer.chrome.com/docs/extensions/develop/concepts/match-patterns>
/// and <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Match_patterns>.
/// Accepts only patterns both Chrome and Firefox load.
fn validate_match_pattern(pattern: &LitStr, kind: &str) -> syn::Result<()> {
    let val = pattern.value();
    let fail = |reason: &str| {
        Err(syn::Error::new(
            pattern.span(),
            format!(
                "invalid {kind} pattern `{val}`: {reason}. See \
                 https://developer.chrome.com/docs/extensions/develop/concepts/match-patterns"
            ),
        ))
    };

    if val == "<all_urls>" {
        return Ok(());
    }

    let missing_scheme = "must start with a scheme (`https://`, `http://`, `*://`, `file://`, \
                          `ftp://`) or be `<all_urls>`";
    let Some((scheme, rest)) = val.split_once("://") else {
        return fail(missing_scheme);
    };
    match scheme {
        "http" | "https" | "*" | "ftp" | "file" => {}
        "ws" | "wss" => {
            return fail("`ws://` and `wss://` work only in Firefox; Chrome rejects them")
        }
        _ => return fail(missing_scheme),
    }

    if val.contains('#') {
        return fail("fragment identifiers (#) are not allowed");
    }
    if val.contains('\\') {
        return fail("backslashes are not allowed");
    }

    let Some(path_start) = rest.find('/') else {
        return fail(
            "must include a path starting with '/' (e.g. `https://example.org/`, \
             `*://*.example.org/*`)",
        );
    };
    if scheme == "file" {
        return Ok(());
    }

    let authority = &rest[..path_start];
    if authority.is_empty() {
        return fail("host cannot be empty");
    }

    let (host, port) = if let Some(inside) = authority.strip_prefix('[') {
        let Some(end) = inside.find(']') else {
            return fail("IPv6 address is missing its closing `]`");
        };
        let host = &authority[..end + 2];
        match &inside[end + 1..] {
            "" => (host, None),
            tail if tail.starts_with(':') => (host, Some(&tail[1..])),
            _ => return fail("unexpected text after the IPv6 address"),
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };

    if host.contains('*') && host != "*" {
        if !host.starts_with('*') {
            return fail("wildcard (*) in host must be at the start");
        }
        match host.strip_prefix("*.") {
            Some(domain) if !domain.is_empty() && !domain.contains('*') => {}
            _ => {
                return fail(
                    "wildcard (*) must be alone or followed by a dot and a domain \
                     (e.g. `*` or `*.example.org`)",
                )
            }
        }
    }

    if let Some(port) = port {
        if port == "*" {
            return fail("a wildcard port works only in Chrome; Firefox rejects it");
        }
        let digits = !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit());
        if !digits || port.parse::<u16>().is_err() {
            return fail(&format!("invalid port `{port}`"));
        }
    }

    Ok(())
}

/// Tries the recommended email-like form first and falls back to a GUID.
///
/// See <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json/browser_specific_settings#id>
fn validate_gecko_id(id: &LitStr) -> syn::Result<()> {
    let val = id.value();

    if is_gecko_id_email(&val) || is_gecko_id_guid(&val) {
        return Ok(());
    }

    Err(syn::Error::new(
        id.span(),
        format!(
            "invalid gecko_id `{val}`. See https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json/browser_specific_settings#id"
        ),
    ))
}

fn is_gecko_id_email(val: &str) -> bool {
    let Some((local, domain)) = val.split_once('@') else {
        return false;
    };

    let is_allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_');

    !domain.is_empty()
        && local.chars().all(is_allowed)
        && domain.chars().all(is_allowed)
        && val.chars().count() <= 80
}

fn is_gecko_id_guid(val: &str) -> bool {
    // The braces are part of the form
    let inner = val
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'));

    inner.is_some_and(|inner| {
        let groups: Vec<&str> = inner.split('-').collect();
        let widths = [8, 4, 4, 4, 12];
        groups.len() == widths.len()
            && groups.iter().zip(widths).all(|(group, width)| {
                group.len() == width && group.chars().all(|c| c.is_ascii_hexdigit())
            })
    })
}

#[derive(Debug)]
pub struct ExtensionArgs {
    pub name: Text,
    pub version: LitStr,
    pub description: Option<Text>,
    pub default_locale: Option<LitStr>,
    pub gecko_id: Option<LitStr>,
    pub permissions: Vec<crate::permission::Declared>,
    pub optional_permissions: Vec<crate::permission::Declared>,
    pub host_permissions: Vec<LitStr>,
    pub optional_host_permissions: Vec<LitStr>,
    pub icons: Vec<(u32, LitStr)>,
    pub action: Option<ActionArgs>,
}

#[derive(Debug)]
pub struct ActionArgs {
    pub default_title: Option<Text>,
    pub default_icon: Option<IconSpec>,
    pub shortcut: Option<ShortcutSpec>,
}

#[derive(Debug)]
pub struct ShortcutSpec {
    pub keys: Vec<(String, LitStr)>,
}

impl ShortcutSpec {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.keys
                .iter()
                .map(|(platform, key)| (platform.clone(), key.value().into()))
                .collect(),
        )
    }
}

fn check_shortcut(key: &LitStr, platform: &str) -> syn::Result<()> {
    crate::shortcut::check(&key.value(), platform)
        .map_err(|reason| syn::Error::new(key.span(), format!("invalid shortcut: {reason}")))
}

pub fn parse_shortcut(input: ParseStream) -> syn::Result<ShortcutSpec> {
    let mut keys: Vec<(String, LitStr)> = Vec::new();
    let mut braces = None;
    if input.peek(LitStr) {
        let key: LitStr = input.parse()?;
        check_shortcut(&key, "default")?;
        keys.push(("default".to_string(), key));
        return finish_shortcut(keys, braces);
    }
    let content;
    braces = Some(syn::braced!(content in input));
    while !content.is_empty() {
        let platform: Ident = content.parse()?;
        let name = platform.to_string();
        if !crate::shortcut::PLATFORMS.contains(&name.as_str()) {
            let reason = match name.as_str() {
                "android" | "ios" => format!("Chrome rejects the extension when `{name}` is listed"),
                _ => format!("unknown platform `{name}`"),
            };
            return Err(syn::Error::new(
                platform.span(),
                format!("{reason}; use `default`, `mac`, `windows`, `linux` or `chromeos`"),
            ));
        }
        if keys.iter().any(|(existing, _)| *existing == name) {
            return Err(syn::Error::new(platform.span(), format!("`{name}` is listed twice")));
        }
        content.parse::<Token![=]>()?;
        let key: LitStr = content.parse()?;
        check_shortcut(&key, &name)?;
        keys.push((name, key));
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }
    finish_shortcut(keys, braces)
}

fn finish_shortcut(
    mut keys: Vec<(String, LitStr)>,
    braces: Option<syn::token::Brace>,
) -> syn::Result<ShortcutSpec> {
    if !keys.iter().any(|(platform, _)| platform == "default") {
        return Err(syn::Error::new(
            braces.expect("a plain shortcut is the default").span.join(),
            "add `default = \"...\"`: Chrome rejects the extension on any platform the shortcut doesn't cover",
        ));
    }
    for platform in ["mac", "windows"] {
        let (from, key) = keys
            .iter()
            .find(|(name, _)| name == platform)
            .or_else(|| keys.iter().find(|(name, _)| name == "default"))
            .unwrap();
        let effective = if platform == "mac" {
            crate::shortcut::effective_mac(&key.value())
        } else {
            key.value()
        };
        if let Some(reason) = crate::shortcut::reserved_by_system(platform, &effective) {
            let fix = if from == "default" {
                format!("add `{platform} = \"...\"` with another combination for {}", if platform == "mac" { "macOS" } else { "Windows" })
            } else {
                "pick another combination".to_string()
            };
            return Err(syn::Error::new(
                key.span(),
                format!("invalid shortcut: on {} `{effective}` is taken: {reason} before the browser sees it; {fix}", if platform == "mac" { "macOS" } else { "Windows" }),
            ));
        }
    }
    let default = keys.iter().position(|(platform, _)| platform == "default").unwrap();
    keys.swap(0, default);
    keys[1..].sort_by_key(|(platform, _)| {
        crate::shortcut::PLATFORMS.iter().position(|known| known == platform)
    });
    Ok(ShortcutSpec { keys })
}

#[derive(Debug)]
pub struct CommandArgs {
    pub description: Text,
    pub shortcut: Option<ShortcutSpec>,
}

pub fn parse_command_args(input: ParseStream) -> syn::Result<CommandArgs> {
    let mut description: Option<Text> = None;
    let mut shortcut: Option<ShortcutSpec> = None;
    let mut seen: Vec<String> = Vec::new();
    while !input.is_empty() {
        let key: Ident = input.parse()?;
        let name = key.to_string();
        if seen.contains(&name) {
            return Err(syn::Error::new(key.span(), format!("`{name}` is given twice")));
        }
        seen.push(name.clone());
        input.parse::<Token![=]>()?;
        match name.as_str() {
            "description" => {
                let text = Text::parse_field(input, "description")?;
                if text.value().trim().is_empty() {
                    return Err(syn::Error::new(
                        text.span(),
                        "the description can't be empty: Chrome rejects the extension and shows it in its shortcut settings",
                    ));
                }
                description = Some(text);
            }
            "shortcut" => shortcut = Some(parse_shortcut(input)?),
            "key" | "suggested_key" | "keys" => {
                return Err(syn::Error::new(key.span(), format!("write `shortcut = \"...\"` instead of `{name}`")))
            }
            "name" | "id" => {
                return Err(syn::Error::new(
                    key.span(),
                    "the command is named after the function; remove `name`",
                ))
            }
            "global" => {
                return Err(syn::Error::new(
                    key.span(),
                    "`global` shortcuts only exist in Chrome; oxichrome doesn't support them",
                ))
            }
            other => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unknown command argument `{other}`; expected `description` or `shortcut`"),
                ))
            }
        }
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    let description = description.ok_or_else(|| {
        syn::Error::new(
            input.span(),
            "add `description = \"...\"`: Chrome requires it and shows it in its shortcut settings",
        )
    })?;
    Ok(CommandArgs { description, shortcut })
}

#[derive(Debug)]
pub enum IconSpec {
    Single(LitStr),
    Sizes(Vec<(u32, LitStr)>),
}

impl IconSpec {
    pub fn paths(&self) -> Vec<&LitStr> {
        match self {
            IconSpec::Single(path) => vec![path],
            IconSpec::Sizes(sizes) => sizes.iter().map(|(_, path)| path).collect(),
        }
    }
}

pub const MAX_ICON_SIZE: u32 = 2048;

fn parse_icon_size(lit: &LitInt) -> syn::Result<u32> {
    let text = lit.to_string();
    let fail = || {
        syn::Error::new(
            lit.span(),
            format!("invalid icon size `{text}`: use a whole number of pixels from 1 to {MAX_ICON_SIZE}, such as `16` or `128`"),
        )
    };
    if !lit.suffix().is_empty()
        || text.starts_with('0')
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(fail());
    }
    let size: u32 = lit.base10_parse().map_err(|_| fail())?;
    if !(1..=MAX_ICON_SIZE).contains(&size) {
        return Err(fail());
    }
    Ok(size)
}

fn parse_icon_map(input: ParseStream) -> syn::Result<Vec<(u32, LitStr)>> {
    let content;
    syn::braced!(content in input);
    let mut sizes: Vec<(u32, LitStr)> = Vec::new();
    while !content.is_empty() {
        if content.peek(LitStr) {
            let key: LitStr = content.parse()?;
            return Err(syn::Error::new(
                key.span(),
                format!(
                    "write the icon size as a number: `{} = \"...\"`",
                    key.value()
                ),
            ));
        }
        let key: LitInt = content.parse()?;
        let size = parse_icon_size(&key)?;
        content.parse::<Token![=]>()?;
        let path: LitStr = content.parse()?;
        normalize_icon_path(&path)?;
        if sizes.iter().any(|(existing, _)| *existing == size) {
            return Err(syn::Error::new(
                key.span(),
                format!("icon size {size} is listed twice"),
            ));
        }
        sizes.push((size, path));
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }
    sizes.sort_by_key(|(size, _)| *size);
    Ok(sizes)
}

fn parse_action(input: ParseStream) -> syn::Result<ActionArgs> {
    let content;
    syn::braced!(content in input);
    let mut action = ActionArgs {
        default_title: None,
        default_icon: None,
        shortcut: None,
    };
    while !content.is_empty() {
        let key: Ident = content.parse()?;
        content.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "default_title" => action.default_title = Some(Text::parse_field(&content, "default_title")?),
            "default_icon" => {
                action.default_icon = Some(if content.peek(LitStr) {
                    let path: LitStr = content.parse()?;
                    normalize_icon_path(&path)?;
                    IconSpec::Single(path)
                } else {
                    IconSpec::Sizes(parse_icon_map(&content)?)
                });
            }
            "shortcut" => action.shortcut = Some(parse_shortcut(&content)?),
            "default_popup" => return Err(syn::Error::new(
                key.span(),
                "the popup comes from your `#[oxichrome::popup]` component; remove `default_popup`",
            )),
            other => return Err(syn::Error::new(
                key.span(),
                format!(
                    "unknown action argument `{other}`; expected `default_title`, `default_icon` or `shortcut`"
                ),
            )),
        }
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }
    Ok(action)
}

pub fn normalize_icon_path(path: &LitStr) -> syn::Result<String> {
    let value = path.value();
    let fail = |reason: &str| {
        Err(syn::Error::new(
            path.span(),
            format!("invalid icon path `{value}`: {reason}"),
        ))
    };
    if value.contains("://") || value.starts_with("data:") {
        return fail("use a PNG file inside `static/`, not a URL");
    }
    if value.contains('\\') {
        return fail("use `/` to separate folders");
    }
    let mut rest = value.as_str();
    loop {
        if let Some(stripped) = rest.strip_prefix("./") {
            rest = stripped;
        } else if let Some(stripped) = rest.strip_prefix('/') {
            rest = stripped;
        } else {
            break;
        }
    }
    if rest.is_empty() {
        return fail("the path is empty");
    }
    if rest
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return fail(
            "the path must stay inside `static/` and can't contain empty, `.` or `..` parts",
        );
    }
    if !rest.to_ascii_lowercase().ends_with(".png") {
        return fail("icons must be PNG files; Chrome can't show SVG or WebP icons");
    }
    Ok(rest.to_string())
}

pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || !bytes.starts_with(SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

impl Parse for ExtensionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<Text> = None;
        let mut version: Option<LitStr> = None;
        let mut description: Option<Text> = None;
        let mut default_locale: Option<LitStr> = None;
        let mut gecko_id: Option<LitStr> = None;
        let mut permissions: Vec<crate::permission::Declared> = Vec::new();
        let mut optional_permissions: Vec<crate::permission::Declared> = Vec::new();
        let mut host_permissions: Vec<LitStr> = Vec::new();
        let mut optional_host_permissions: Vec<LitStr> = Vec::new();
        let mut icons: Vec<(u32, LitStr)> = Vec::new();
        let mut action: Option<ActionArgs> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "name" => {
                    name = Some(Text::parse_field(input, "name")?);
                }
                "default_locale" => {
                    default_locale = Some(input.parse()?);
                }
                "version" => {
                    version = Some(input.parse()?);
                }
                "description" => {
                    description = Some(Text::parse_field(input, "description")?);
                }
                "gecko_id" => {
                    let id: LitStr = input.parse()?;
                    validate_gecko_id(&id)?;
                    gecko_id = Some(id);
                }
                "permissions" => {
                    permissions = crate::permission::parse_list(input, "permissions")?;
                }
                "optional_permissions" => {
                    optional_permissions = crate::permission::parse_list(input, "optional_permissions")?;
                }
                "host_permissions" | "optional_host_permissions" => {
                    let list = if key == "host_permissions" { &mut host_permissions } else { &mut optional_host_permissions };
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        if content.peek(Ident) {
                            let ident: Ident = content.parse()?;
                            return Err(syn::Error::new(
                                ident.span(),
                                format!("`{key}` takes site patterns like \"https://example.com/*\"; put `{ident}` in `{}`", if key == "host_permissions" { "permissions" } else { "optional_permissions" }),
                            ));
                        }
                        let pattern: LitStr = content.parse()?;
                        validate_match_pattern(&pattern, "host_permission")?;
                        if list.iter().any(|seen| seen.value() == pattern.value()) {
                            return Err(syn::Error::new(pattern.span(), format!("`{}` is listed twice in `{key}`", pattern.value())));
                        }
                        list.push(pattern);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "icons" => {
                    icons = parse_icon_map(input)?;
                }
                "action" => {
                    action = Some(parse_action(input)?);
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown argument `{other}`"),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let name = name.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required argument `name`",
            )
        })?;
        let version = version.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required argument `version`",
            )
        })?;

        for optional in &optional_permissions {
            if permissions.iter().any(|required| required.info.ident == optional.info.ident) {
                return Err(syn::Error::new(
                    optional.ident.span(),
                    format!("`{}` is in `permissions` too; a permission is either granted at install or asked for later, so keep it in one list", optional.ident),
                ));
            }
        }
        for optional in &optional_host_permissions {
            if host_permissions.iter().any(|required| required.value() == optional.value()) {
                return Err(syn::Error::new(
                    optional.span(),
                    format!("`{}` is in `host_permissions` too; keep it in one list", optional.value()),
                ));
            }
        }

        Ok(ExtensionArgs {
            name,
            version,
            description,
            default_locale,
            gecko_id,
            permissions,
            optional_permissions,
            host_permissions,
            optional_host_permissions,
            icons,
            action,
        })
    }
}

#[derive(Debug)]
pub struct EventArgs {
    pub namespace: Ident,
    pub event_name: Ident,
}

impl Parse for EventArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let namespace: Ident = input.parse()?;
        input.parse::<Token![::]>()?;
        let event_name: Ident = input.parse()?;

        Ok(EventArgs {
            namespace,
            event_name,
        })
    }
}

#[derive(Debug)]
pub struct ContentScriptArgs {
    pub on_demand: bool,
    pub matches: Vec<LitStr>,
    pub run_at: Option<Ident>,
    pub all_frames: Option<bool>,
    pub css: Vec<LitStr>,
}

impl Parse for ContentScriptArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut matches: Vec<LitStr> = Vec::new();
        let mut run_at: Option<Ident> = None;
        let mut all_frames: Option<bool> = None;
        let mut css: Vec<LitStr> = Vec::new();
        let mut matches_key: Option<Ident> = None;
        let mut extras: Vec<Ident> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if key != "matches" {
                extras.push(key.clone());
            }

            match key.to_string().as_str() {
                "matches" => {
                    matches_key = Some(key.clone());
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        matches.push(content.parse()?);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "run_at" => {
                    let value: Ident = input.parse()?;
                    run_at = Some(value);
                }
                "all_frames" => {
                    let value: LitBool = input.parse()?;
                    all_frames = Some(value.value());
                }
                "css" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        css.push(content.parse()?);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown argument `{other}`"),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        if let Some(key) = &matches_key {
            if matches.is_empty() {
                return Err(syn::Error::new(
                    key.span(),
                    "list at least one pattern, or remove `matches` to make a script you run on demand with `SCRIPT.run`",
                ));
            }
        } else if let Some(extra) = extras.first() {
            return Err(syn::Error::new(
                extra.span(),
                format!("`{extra}` only applies to scripts that run on matching pages; add `matches = [...]`, or remove `{extra}` for a script you run on demand"),
            ));
        }

        for pattern in &matches {
            validate_match_pattern(pattern, "match")?;
        }

        Ok(ContentScriptArgs {
            on_demand: matches_key.is_none(),
            matches,
            run_at,
            all_frames,
            css,
        })
    }
}

pub const MENU_CONTEXTS: &[&str] = &[
    "page", "frame", "selection", "link", "editable", "image", "video", "audio", "action",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuKind {
    Normal,
    Checkbox(bool),
    Radio(bool),
}

#[derive(Debug)]
pub struct MenuArgs {
    pub title: Text,
    pub contexts: Option<Vec<Ident>>,
    pub kind: MenuKind,
    pub parent: Option<syn::Path>,
    pub enabled: Option<bool>,
    pub visible: Option<bool>,
    pub document_url_patterns: Vec<LitStr>,
    pub target_url_patterns: Vec<LitStr>,
}

fn parse_patterns(input: ParseStream, kind: &str) -> syn::Result<Vec<LitStr>> {
    let content;
    syn::bracketed!(content in input);
    let mut patterns = Vec::new();
    while !content.is_empty() {
        let pattern: LitStr = content.parse()?;
        validate_match_pattern(&pattern, kind)?;
        patterns.push(pattern);
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }
    Ok(patterns)
}

pub fn parse_menu_args(input: ParseStream, submenu: bool) -> syn::Result<MenuArgs> {
    let attr = if submenu { "submenu" } else { "menu" };
    let mut title: Option<Text> = None;
    let mut contexts: Option<Vec<Ident>> = None;
    let mut kind = MenuKind::Normal;
    let mut kind_key: Option<Ident> = None;
    let mut parent = None;
    let mut enabled = None;
    let mut visible = None;
    let mut document_url_patterns = Vec::new();
    let mut target_url_patterns = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    while !input.is_empty() {
        let key: Ident = input.call(syn::ext::IdentExt::parse_any)?;
        let name = key.to_string();
        if seen.contains(&name) {
            return Err(syn::Error::new(key.span(), format!("`{name}` is given twice")));
        }
        seen.push(name.clone());
        input.parse::<Token![=]>()?;
        match name.as_str() {
            "title" => title = Some(Text::parse_field(input, "title")?),
            "contexts" => {
                let content;
                let brackets = syn::bracketed!(content in input);
                let mut list: Vec<Ident> = Vec::new();
                while !content.is_empty() {
                    if content.peek(LitStr) {
                        let lit: LitStr = content.parse()?;
                        return Err(syn::Error::new(
                            lit.span(),
                            format!("write contexts without quotes: `contexts = [{}]`", lit.value()),
                        ));
                    }
                    let context: Ident = content.parse()?;
                    let text = context.to_string();
                    if text == "all" {
                        return Err(syn::Error::new(
                            context.span(),
                            "`all` means different things in Chrome and Firefox (Firefox includes the toolbar button); list the contexts you want, such as `contexts = [page, link, selection]`",
                        ));
                    }
                    if !MENU_CONTEXTS.contains(&text.as_str()) {
                        return Err(syn::Error::new(
                            context.span(),
                            format!(
                                "unknown context `{text}`; expected one of {}. Other contexts work in only one browser",
                                MENU_CONTEXTS.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ")
                            ),
                        ));
                    }
                    if list.iter().any(|existing| *existing == text) {
                        return Err(syn::Error::new(
                            context.span(),
                            format!("context `{text}` is listed twice"),
                        ));
                    }
                    list.push(context);
                    if !content.is_empty() {
                        content.parse::<Token![,]>()?;
                    }
                }
                if list.is_empty() {
                    return Err(syn::Error::new(
                        brackets.span.join(),
                        "list at least one context, or leave `contexts` out to use `[page]`",
                    ));
                }
                contexts = Some(list);
            }
            "checkbox" | "radio" if !submenu => {
                if let Some(other) = &kind_key {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("an item can't be both `{other}` and `{name}`"),
                    ));
                }
                let checked: LitBool = input.parse()?;
                kind = if name == "checkbox" {
                    MenuKind::Checkbox(checked.value())
                } else {
                    MenuKind::Radio(checked.value())
                };
                kind_key = Some(key);
            }
            "checkbox" | "radio" => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("a submenu can't be a {name}; put the {name} items inside it"),
                ))
            }
            "parent" => parent = Some(input.parse()?),
            "enabled" => enabled = Some(input.parse::<LitBool>()?.value()),
            "visible" => visible = Some(input.parse::<LitBool>()?.value()),
            "document_url_patterns" => {
                document_url_patterns = parse_patterns(input, "document_url_patterns")?
            }
            "target_url_patterns" => {
                target_url_patterns = parse_patterns(input, "target_url_patterns")?
            }
            "id" => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("the id is the {} name; remove `id`", if submenu { "struct" } else { "function" }),
                ))
            }
            "type" | "checked" => {
                return Err(syn::Error::new(
                    key.span(),
                    "write `checkbox = false` or `radio = false` (the value is whether it starts checked)",
                ))
            }
            other => {
                let mut expected = vec!["title", "contexts"];
                if !submenu {
                    expected.extend(["checkbox", "radio"]);
                }
                expected.extend(["parent", "enabled", "visible", "document_url_patterns", "target_url_patterns"]);
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "unknown {attr} argument `{other}`; expected {}",
                        expected.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
        }
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }

    let title = title.ok_or_else(|| {
        syn::Error::new(proc_macro2::Span::call_site(), "missing required argument `title`")
    })?;
    if title.value().trim().is_empty() {
        return Err(syn::Error::new(title.span(), "the title can't be empty"));
    }
    let names: Vec<String> = contexts
        .iter()
        .flatten()
        .map(|context| context.to_string())
        .collect();
    if title.texts().iter().any(|text| text.contains("%s")) && names != ["selection"] {
        return Err(syn::Error::new(
            title.span(),
            "`%s` is replaced with the selected text only when `contexts = [selection]`; in any other context browsers show `%s` as is",
        ));
    }
    if !target_url_patterns.is_empty() {
        let media = ["link", "image", "video", "audio"];
        if names.is_empty() || names.iter().any(|name| !media.contains(&name.as_str())) {
            return Err(syn::Error::new(
                target_url_patterns[0].span(),
                "`target_url_patterns` only filter links and media; set `contexts` to some of `link`, `image`, `video`, `audio`",
            ));
        }
    }

    Ok(MenuArgs {
        title,
        contexts,
        kind,
        parent,
        enabled,
        visible,
        document_url_patterns,
        target_url_patterns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extension(tokens: proc_macro2::TokenStream) -> syn::Result<ExtensionArgs> {
        syn::parse2::<ExtensionArgs>(quote::quote! { name = "T", version = "1.0.0", #tokens })
    }

    #[test]
    fn icon_sizes_follow_both_browsers() {
        let args =
            extension(quote::quote! { icons = { 128 = "b.png", 16 = "a.png", 2048 = "c.png" } })
                .unwrap();
        let sizes: Vec<u32> = args.icons.iter().map(|(size, _)| *size).collect();
        assert_eq!(sizes, vec![16, 128, 2048]);
        for bad in ["0", "2049", "016", "16u32", "99999"] {
            let size: proc_macro2::TokenStream = bad.parse().unwrap();
            let err = extension(quote::quote! { icons = { #size = "a.png" } }).expect_err(bad);
            assert!(
                err.to_string().contains("invalid icon size"),
                "{bad}: {err}"
            );
        }
        let err = extension(quote::quote! { icons = { "16" = "a.png" } }).unwrap_err();
        assert!(
            err.to_string().contains("write the icon size as a number"),
            "{err}"
        );
        let err = extension(quote::quote! { icons = { 16 = "a.png", 16 = "b.png" } }).unwrap_err();
        assert!(err.to_string().contains("listed twice"), "{err}");
    }

    #[test]
    fn icon_paths_stay_inside_static() {
        let lit = |value: &str| LitStr::new(value, proc_macro2::Span::call_site());
        for (input, expected) in [
            ("a.png", "a.png"),
            ("/a.png", "a.png"),
            ("./icons/a.png", "icons/a.png"),
            ("icons/A.PNG", "icons/A.PNG"),
        ] {
            assert_eq!(normalize_icon_path(&lit(input)).unwrap(), expected);
        }
        for bad in [
            "",
            "/",
            "../a.png",
            "icons/../a.png",
            "icons//a.png",
            "icons\\a.png",
            "https://example.org/a.png",
            "data:image/png;base64,AA",
            "a.svg",
            "a.webp",
            "a",
        ] {
            assert!(normalize_icon_path(&lit(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn png_dimensions_reads_the_header() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&16u32.to_be_bytes());
        png.extend_from_slice(&32u32.to_be_bytes());
        assert_eq!(png_dimensions(&png), Some((16, 32)));
        assert_eq!(png_dimensions(b"not an image at all, not a png"), None);
        let mut zero = png.clone();
        zero[16..20].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(png_dimensions(&zero), None);
        assert_eq!(png_dimensions(&png[..20]), None);
    }

    #[test]
    fn action_args_are_checked() {
        let args =
            extension(quote::quote! { action = { default_title = "Hi", default_icon = "a.png" } })
                .unwrap();
        let action = args.action.unwrap();
        assert_eq!(action.default_title.unwrap().value(), "Hi");
        assert!(matches!(action.default_icon, Some(IconSpec::Single(_))));
        assert!(extension(quote::quote! { action = {} })
            .unwrap()
            .action
            .is_some());
        let err = extension(quote::quote! { action = { default_popup = "p.html" } }).unwrap_err();
        assert!(err.to_string().contains("#[oxichrome::popup]"), "{err}");
        let err = extension(quote::quote! { action = { colour = "red" } }).unwrap_err();
        assert!(
            err.to_string().contains("unknown action argument `colour`"),
            "{err}"
        );
        let err =
            extension(quote::quote! { action = { default_icon = { 16 = "a.svg" } } }).unwrap_err();
        assert!(err.to_string().contains("must be PNG"), "{err}");
    }

    #[test]
    fn parse_extension_args() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test Extension",
            version = "1.0.0",
            permissions = [Storage, Tabs]
        };
        let args: ExtensionArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.name.value(), "Test Extension");
        assert_eq!(args.version.value(), "1.0.0");
        assert_eq!(args.permissions.len(), 2);
        assert_eq!(args.host_permissions.len(), 0);
    }

    #[test]
    fn parse_extension_args_with_host_permissions() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test Extension",
            version = "1.0.0",
            permissions = [Storage],
            host_permissions = ["*://developer.mozilla.org/*", "*://*.example.org/*"]
        };
        let args: ExtensionArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.name.value(), "Test Extension");
        assert_eq!(args.host_permissions.len(), 2);
        assert_eq!(
            args.host_permissions[0].value(),
            "*://developer.mozilla.org/*"
        );
        assert_eq!(args.host_permissions[1].value(), "*://*.example.org/*");
    }

    #[test]
    fn parse_extension_args_host_permissions_invalid_pattern() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test Extension",
            version = "1.0.0",
            host_permissions = ["invalid-pattern"]
        };
        let err = syn::parse2::<ExtensionArgs>(tokens).unwrap_err();
        assert!(err.to_string().contains("must start with a scheme"));
    }

    #[test]
    fn parse_extension_args_host_permissions_all_urls() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test Extension",
            version = "1.0.0",
            host_permissions = ["<all_urls>"]
        };
        let args: ExtensionArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.host_permissions.len(), 1);
        assert_eq!(args.host_permissions[0].value(), "<all_urls>");
    }

    #[test]
    fn parse_host_permissions_valid_patterns() {
        let valid_patterns = vec![
            "<all_urls>",
            "*://*/*",
            "https://*/*",
            "*://*.mozilla.org/*",
            "*://mozilla.org/",
            "https://*/path",
            "https://*/path/",
            "https://mozilla.org/*",
            "https://mozilla.org/a/b/c/",
            "https://mozilla.org/*/b/*/",
            "file:///blah/*",
            "ftp://mozilla.org/",
            "http://127.0.0.1/*",
            "https://*.google.com/foo*bar",
            "https://*/foo*",
            "file:///*",
            "file:///",
            "file:///C:/*",
            "file://somehost/path",
            "file://*/*",
            "http://127.0.0.1:8080/*",
            "http://a.com:0/*",
            "http://a.com:65535/*",
            "https://[::1]/*",
            "https://[::1]:8080/*",
            "https://*/",
            "https://a.com/p?q=1",
        ];

        for pattern in valid_patterns {
            let tokens: proc_macro2::TokenStream = quote::quote! {
                name = "Test",
                version = "1.0.0",
                host_permissions = [#pattern]
            };
            let args = syn::parse2::<ExtensionArgs>(tokens)
                .unwrap_or_else(|e| panic!("Pattern '{}' should be valid: {}", pattern, e));
            assert_eq!(args.host_permissions[0].value(), pattern);
        }
    }

    #[test]
    fn parse_host_permissions_invalid_patterns() {
        let invalid_patterns = vec![
            ("invalid-pattern", "missing scheme"),
            ("https://mozilla.org", "missing path"),
            ("https://mozilla.*.org/", "wildcard in middle of host"),
            (
                "https://*zilla.org/",
                "wildcard must be alone or followed by dot",
            ),
            ("http*://mozilla.org/", "wildcard in scheme"),
            ("*://*", "empty path"),
            ("resource://path/", "unsupported scheme"),
            ("https://www.mozilla.org/#section1", "fragment identifier"),
            ("https:///path/", "missing host"),
            ("file:/*", "missing slashes"),
            ("http://*:*/*", "wildcard port"),
            ("http://127.0.0.1:*/*", "wildcard port"),
            ("http://a.com:/*", "empty port"),
            ("http://a.com:65536/*", "port out of range"),
            ("http://a.com:+80/*", "signed port"),
            ("https://a.com:80:80/*", "two ports"),
            ("https://[::1/*", "unclosed IPv6"),
            ("https://[::1]x/*", "text after IPv6"),
            ("https://*.*/*", "two wildcards"),
            ("https://*./*", "wildcard without domain"),
            ("https://a.com\\x/*", "backslash"),
            ("ws://example.org/*", "Firefox only"),
            ("wss://example.org/*", "Firefox only"),
            ("HTTPS://a.com/*", "uppercase scheme"),
            ("urn:*", "unsupported scheme"),
        ];

        for (pattern, _reason) in invalid_patterns {
            let tokens: proc_macro2::TokenStream = quote::quote! {
                name = "Test",
                version = "1.0.0",
                host_permissions = [#pattern]
            };
            let err = syn::parse2::<ExtensionArgs>(tokens)
                .expect_err(&format!("Pattern '{}' should be invalid", pattern));
            assert!(
                err.to_string().contains("invalid host_permission pattern")
                    || err.to_string().contains("must start with a scheme"),
                "Expected scheme error for '{}', got: {}",
                pattern,
                err
            );
        }
    }

    #[test]
    fn parse_extension_args_with_gecko_id() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test",
            version = "1.0.0",
            gecko_id = "test@example.com"
        };
        let args: ExtensionArgs = syn::parse2(tokens).unwrap();
        // More so ensuring we didn't break anything, as the value isn't stored in the ExtensionArgs
        assert_eq!(args.name.value(), "Test");
        assert_eq!(args.version.value(), "1.0.0");
    }

    #[test]
    fn parse_extension_args_gecko_id_guid() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test",
            version = "1.0.0",
            gecko_id = "{daf44bf7-a45e-4450-979c-91cf07434c3d}"
        };
        let args: ExtensionArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.name.value(), "Test");
    }

    #[test]
    fn parse_extension_args_gecko_id_invalid() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test",
            version = "1.0.0",
            gecko_id = "not-an-id"
        };
        let err = syn::parse2::<ExtensionArgs>(tokens).unwrap_err();
        assert!(err.to_string().contains("invalid gecko_id `not-an-id`"));
    }

    fn gecko_id(val: &str) -> LitStr {
        LitStr::new(val, proc_macro2::Span::call_site())
    }

    #[test]
    fn validate_gecko_id_accepts_either_form() {
        assert!(validate_gecko_id(&gecko_id("my-ext@example.com")).is_ok());
        assert!(validate_gecko_id(&gecko_id("{daf44bf7-a45e-4450-979c-91cf07434c3d}")).is_ok());
    }

    #[test]
    fn validate_gecko_id_error_points_at_the_docs() {
        for val in ["not-an-id", "{not-a-guid}", "my ext@example.com"] {
            let err = validate_gecko_id(&gecko_id(val)).unwrap_err().to_string();
            assert!(
                err.contains(val),
                "Expected the offending value in the error for '{val}', got: {err}"
            );
            assert!(
                err.contains("browser_specific_settings#id"),
                "Expected a docs link in the error for '{val}', got: {err}"
            );
        }
    }

    #[test]
    fn is_gecko_id_guid_accepts_hex_groups() {
        // Hex is case-insensitive
        for val in [
            "{daf44bf7-a45e-4450-979c-91cf07434c3d}",
            "{DAF44BF7-A45E-4450-979C-91CF07434C3D}",
        ] {
            assert!(is_gecko_id_guid(val), "Expected '{val}' to be accepted");
        }
    }

    #[test]
    fn is_gecko_id_guid_rejects_malformed() {
        for val in [
            "daf44bf7-a45e-4450-979c-91cf07434c3d",  // braces are required
            "{daf44bf7-a45e-4450-979c-91cf07434c3d", // unterminated
            "{daf44bf7-a45e-4450}",                  // too few groups
            "{daf44bf7a45e4450979c91cf07434c3d}",    // no separators
            "{daf44bf7-a45e-4450-979c-91cf07434c3}", // short final group
            "{daf44bf7-a45e-4450-979c-91cf07434cgg}", // non-hex digits
            "{}",
        ] {
            assert!(!is_gecko_id_guid(val), "Expected '{val}' to be rejected");
        }
    }

    #[test]
    fn is_gecko_id_email_accepts_allowed_chars() {
        for val in [
            "my-ext@example.com",
            "great_app@developers.company",
            "a.b-c_d@e.f-g_h",
            // Regex on MDN allows an empty user
            "@example.com",
        ] {
            assert!(is_gecko_id_email(val), "Expected '{val}' to be accepted");
        }
    }

    #[test]
    fn is_gecko_id_email_rejects_malformed() {
        for val in [
            "not-an-id",           // no `@`
            "my-ext@",             // empty domain
            "my ext@example.com",  // space
            "my+ext@example.com",  // plus
            "my@ext@example.com",  // second `@`
            "my-ext@exam ple.com", // space in the domain
        ] {
            assert!(!is_gecko_id_email(val), "Expected '{val}' to be rejected");
        }
    }

    #[test]
    fn is_gecko_id_email_rejects_over_80_chars() {
        // 80 characters exactly is still fine
        let at_limit = format!("{}@example.com", "a".repeat(68));
        assert_eq!(at_limit.chars().count(), 80);
        assert!(is_gecko_id_email(&at_limit));

        let too_long = format!("{}@example.com", "a".repeat(69));
        assert!(!is_gecko_id_email(&too_long));
    }

    #[test]
    fn parse_event_args() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            runtime::on_installed
        };
        let args: EventArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.namespace.to_string(), "runtime");
        assert_eq!(args.event_name.to_string(), "on_installed");
    }

    #[test]
    fn parse_content_script_args() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["<all_urls>"]
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.matches.len(), 1);
        assert_eq!(args.matches[0].value(), "<all_urls>");
    }

    #[test]
    fn parse_content_script_args_multiple() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["https://example.com/*", "https://test.com/*"]
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.matches.len(), 2);
        assert_eq!(args.matches[0].value(), "https://example.com/*");
        assert_eq!(args.matches[1].value(), "https://test.com/*");
    }

    #[test]
    fn parse_content_script_args_wildcard_scheme() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["*://*.youtube.com/*"]
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.matches[0].value(), "*://*.youtube.com/*");
    }

    #[test]
    fn parse_content_script_args_invalid_pattern() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["youtube.com/*"]
        };
        let err = syn::parse2::<ContentScriptArgs>(tokens).unwrap_err();
        assert!(err.to_string().contains("must start with a scheme"));
    }

    #[test]
    fn parse_content_script_args_checks_the_whole_pattern() {
        for pattern in ["file:///*", "https://[::1]:8080/*", "*://*.example.org/*"] {
            let tokens = quote::quote! { matches = [#pattern] };
            assert!(
                syn::parse2::<ContentScriptArgs>(tokens).is_ok(),
                "{pattern}"
            );
        }
        for pattern in [
            "http://*:*/*",
            "https://mozilla.org",
            "https://*.*/*",
            "ws://a.org/*",
        ] {
            let tokens = quote::quote! { matches = [#pattern] };
            let err = syn::parse2::<ContentScriptArgs>(tokens).expect_err(pattern);
            assert!(err.to_string().contains("invalid match pattern"), "{err}");
        }
    }

    #[test]
    fn parse_content_script_args_with_run_at() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["<all_urls>"],
            run_at = DocumentStart
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.run_at.unwrap().to_string(), "DocumentStart");
    }

    #[test]
    fn parse_content_script_args_with_all_frames() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["<all_urls>"],
            all_frames = true
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.all_frames, Some(true));
    }

    #[test]
    fn parse_content_script_args_with_css() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["<all_urls>"],
            css = ["styles.css", "theme.css"]
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.css.len(), 2);
        assert_eq!(args.css[0].value(), "styles.css");
        assert_eq!(args.css[1].value(), "theme.css");
    }

    #[test]
    fn parse_content_script_args_all_options() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            matches = ["<all_urls>"],
            run_at = DocumentStart,
            all_frames = true,
            css = ["styles.css"]
        };
        let args: ContentScriptArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.matches.len(), 1);
        assert_eq!(args.run_at.unwrap().to_string(), "DocumentStart");
        assert_eq!(args.all_frames, Some(true));
        assert_eq!(args.css.len(), 1);
    }

    fn menu(tokens: proc_macro2::TokenStream) -> syn::Result<MenuArgs> {
        syn::parse::Parser::parse2(|input: ParseStream| parse_menu_args(input, false), tokens)
    }

    fn submenu(tokens: proc_macro2::TokenStream) -> syn::Result<MenuArgs> {
        syn::parse::Parser::parse2(|input: ParseStream| parse_menu_args(input, true), tokens)
    }

    fn menu_err(tokens: proc_macro2::TokenStream) -> String {
        menu(tokens).unwrap_err().to_string()
    }

    #[test]
    fn menu_args_are_checked() {
        let args = menu(quote::quote! { title = "Search %s", contexts = [selection], enabled = false }).unwrap();
        assert_eq!(args.contexts.unwrap().len(), 1);
        assert_eq!(args.enabled, Some(false));
        assert_eq!(menu(quote::quote! { title = "T", checkbox = true }).unwrap().kind, MenuKind::Checkbox(true));
        assert_eq!(menu(quote::quote! { title = "T", radio = false }).unwrap().kind, MenuKind::Radio(false));
        assert!(menu(quote::quote! { title = "T", parent = tools::Tools }).unwrap().parent.is_some());

        for (tokens, expected) in [
            (quote::quote! {}, "missing required argument `title`"),
            (quote::quote! { title = " " }, "can't be empty"),
            (quote::quote! { title = "Search %s" }, "contexts = [selection]"),
            (quote::quote! { title = "Search %s", contexts = [selection, link] }, "contexts = [selection]"),
            (quote::quote! { title = "T", contexts = [all] }, "`all` means different things"),
            (quote::quote! { title = "T", contexts = [tab] }, "unknown context `tab`"),
            (quote::quote! { title = "T", contexts = [bookmark] }, "only one browser"),
            (quote::quote! { title = "T", contexts = ["page"] }, "without quotes"),
            (quote::quote! { title = "T", contexts = [page, page] }, "listed twice"),
            (quote::quote! { title = "T", contexts = [] }, "at least one context"),
            (quote::quote! { title = "T", checkbox = true, radio = true }, "both `checkbox` and `radio`"),
            (quote::quote! { title = "T", title = "U" }, "given twice"),
            (quote::quote! { title = "T", id = "x" }, "the id is the function name"),
            (quote::quote! { title = "T", type = "checkbox" }, "checkbox = false"),
            (quote::quote! { title = "T", checked = true }, "checkbox = false"),
            (quote::quote! { title = "T", icons = {} }, "unknown menu argument `icons`"),
            (quote::quote! { title = "T", document_url_patterns = ["nope"] }, "invalid document_url_patterns pattern"),
            (quote::quote! { title = "T", target_url_patterns = ["https://a.org/*"] }, "only filter links and media"),
            (quote::quote! { title = "T", contexts = [page, link], target_url_patterns = ["https://a.org/*"] }, "only filter links and media"),
        ] {
            let err = menu_err(tokens.clone());
            assert!(err.contains(expected), "{tokens}: {err}");
        }
        assert!(menu(quote::quote! { title = "T", contexts = [link, image], target_url_patterns = ["https://a.org/*"] }).is_ok());
    }

    #[test]
    fn submenus_take_no_item_kind() {
        assert!(submenu(quote::quote! { title = "Tools", contexts = [page, link] }).is_ok());
        let err = submenu(quote::quote! { title = "Tools", checkbox = true }).unwrap_err().to_string();
        assert!(err.contains("a submenu can't be a checkbox"), "{err}");
        let err = submenu(quote::quote! { title = "Tools", id = "x" }).unwrap_err().to_string();
        assert!(err.contains("the struct name"), "{err}");
    }

    #[test]
    fn content_script_without_matches_runs_on_demand() {
        let args: ContentScriptArgs = syn::parse2(quote::quote! {}).unwrap();
        assert!(args.on_demand);
        let args: ContentScriptArgs = syn::parse2(quote::quote! { matches = ["<all_urls>"], run_at = DocumentEnd }).unwrap();
        assert!(!args.on_demand);
        for (tokens, expected) in [
            (quote::quote! { matches = [] }, "list at least one pattern, or remove `matches`"),
            (quote::quote! { run_at = DocumentEnd }, "`run_at` only applies to scripts that run on matching pages"),
            (quote::quote! { css = ["a.css"] }, "`css` only applies"),
            (quote::quote! { all_frames = true }, "`all_frames` only applies"),
        ] {
            let err = syn::parse2::<ContentScriptArgs>(tokens).err().unwrap().to_string();
            assert!(err.contains(expected), "{err}");
        }
    }

    #[test]
    fn extension_permission_lists_cross_checked() {
        let args: ExtensionArgs = syn::parse2(quote::quote! {
            name = "T", version = "1.0.0", permissions = [Storage], optional_permissions = [History],
            host_permissions = ["https://a.com/*"], optional_host_permissions = ["https://b.com/*"]
        }).unwrap();
        assert_eq!(args.optional_permissions[0].info.name, "history");
        assert_eq!(args.optional_host_permissions[0].value(), "https://b.com/*");
        for (tokens, expected) in [
            (quote::quote! { name = "T", version = "1", permissions = [Tabs], optional_permissions = [Tabs] }, "`Tabs` is in `permissions` too"),
            (quote::quote! { name = "T", version = "1", host_permissions = ["https://a.com/*"], optional_host_permissions = ["https://a.com/*"] }, "is in `host_permissions` too"),
            (quote::quote! { name = "T", version = "1", host_permissions = ["https://a.com/*", "https://a.com/*"] }, "listed twice"),
            (quote::quote! { name = "T", version = "1", host_permissions = [Tabs] }, "put `Tabs` in `permissions`"),
            (quote::quote! { name = "T", version = "1", optional_host_permissions = [History] }, "put `History` in `optional_permissions`"),
        ] {
            let err = syn::parse2::<ExtensionArgs>(tokens).err().unwrap().to_string();
            assert!(err.contains(expected), "{err}");
        }
    }
}
