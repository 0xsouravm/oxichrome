use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const SECTION: &str = "oxichrome.v1";

#[derive(Debug, Default)]
pub struct ExtensionMetadata {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub default_locale: Option<String>,
    pub messages: Vec<UsedMessage>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub optional_permissions: Vec<String>,
    pub host_permissions: Vec<String>,
    pub optional_host_permissions: Vec<String>,
    pub chrome_only: Vec<(String, String)>,
    pub firefox_only: Vec<(String, String)>,
    pub background_exports: Vec<String>,
    pub event_handlers: Vec<EventHandler>,
    pub has_popup: bool,
    pub has_options_page: bool,
    pub popup_styles: Vec<String>,
    pub options_styles: Vec<String>,
    pub side_panel: Option<SidePanel>,
    pub omnibox: Option<Omnibox>,
    pub content_scripts: Vec<ContentScript>,
    pub gecko_id: Option<String>,
    pub icons: Vec<(u32, String)>,
    pub action: Option<ActionMetadata>,
    pub menus: Vec<MenuItem>,
    pub commands: Vec<Command>,
    pub rule_sets: Vec<RuleSet>,
    pub web_accessible: Vec<String>,
    pub offscreen: Vec<OffscreenFn>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OffscreenFn {
    pub name: String,
    pub export: String,
    pub reasons: Vec<String>,
    pub justification: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RuleSet {
    pub id: String,
    pub path: String,
    pub enabled: bool,
}

pub type Shortcut = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Omnibox {
    pub fn_name: String,
    pub keyword: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct SidePanel {
    pub title: Option<String>,
    #[serde(default)]
    pub open_on_action_click: bool,
    #[serde(default)]
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct UsedMessage {
    pub key: String,
    pub args: usize,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub name: String,
    pub export: String,
    pub description: String,
    pub shortcut: Option<Shortcut>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommandEntry {
    name: String,
    export: String,
    description: String,
    shortcut: Option<Shortcut>,
    file: String,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub id: String,
    pub handler: Option<String>,
    pub export: Option<String>,
    pub item_type: String,
    pub title: String,
    pub contexts: Vec<String>,
    pub checked: Option<bool>,
    pub parent: Option<String>,
    pub enabled: Option<bool>,
    pub visible: Option<bool>,
    pub document_url_patterns: Vec<String>,
    pub target_url_patterns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MenuEntry {
    id: String,
    handler: Option<String>,
    export: Option<String>,
    item_type: String,
    title: String,
    contexts: Option<Vec<String>>,
    checked: Option<bool>,
    parent: Option<String>,
    enabled: Option<bool>,
    visible: Option<bool>,
    document_url_patterns: Vec<String>,
    target_url_patterns: Vec<String>,
    file: String,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ActionMetadata {
    pub default_title: Option<String>,
    pub default_icon: Option<ActionIcon>,
    #[serde(default)]
    pub shortcut: Option<Shortcut>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ActionIcon {
    Single(String),
    Sizes(Vec<(u32, String)>),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EventHandler {
    pub fn_name: String,
    pub export: String,
    pub namespace: String,
    pub event_name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ContentScript {
    pub fn_name: String,
    pub export: String,
    pub matches: Vec<String>,
    pub run_at: Option<String>,
    pub all_frames: Option<bool>,
    #[serde(default)]
    pub css: Vec<String>,
}

impl ContentScript {
    pub fn file_name(&self) -> String {
        format!("content_script_{}.js", self.export.trim_start_matches("__oxichrome_cs_"))
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Entry {
    Extension {
        name: String,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        default_locale: Option<String>,
        version: String,
        description: Option<String>,
        gecko_id: Option<String>,
        permissions: Vec<String>,
        #[serde(default)]
        optional_permissions: Vec<String>,
        host_permissions: Vec<String>,
        #[serde(default)]
        optional_host_permissions: Vec<String>,
        #[serde(default)]
        chrome_only: Vec<(String, String)>,
        #[serde(default)]
        firefox_only: Vec<(String, String)>,
        icons: Vec<(u32, String)>,
        action: Option<ActionMetadata>,
        #[serde(default)]
        rule_sets: Vec<RuleSet>,
        #[serde(default)]
        web_accessible: Vec<String>,
    },
    Background {
        fn_name: String,
        export: String,
    },
    Message(UsedMessage),
    Event {
        fn_name: String,
        export: String,
        namespace: String,
        event_name: String,
    },
    Popup {
        fn_name: String,
        #[serde(default)]
        styles: Vec<String>,
    },
    OptionsPage {
        fn_name: String,
        #[serde(default)]
        styles: Vec<String>,
    },
    SidePanel {
        fn_name: String,
        #[serde(flatten)]
        panel: SidePanel,
    },
    Omnibox(Omnibox),
    ContentScript {
        fn_name: String,
        export: String,
        matches: Vec<String>,
        run_at: Option<String>,
        all_frames: Option<bool>,
        css: Vec<String>,
    },
    Menu(MenuEntry),
    Command(CommandEntry),
    Offscreen(OffscreenFn),
    Script {
        name: String,
        file: String,
        line: usize,
        column: usize,
    },
}

fn read_leb128(bytes: &[u8], pos: &mut usize) -> anyhow::Result<usize> {
    let mut value: u64 = 0;
    let mut shift = 0;
    loop {
        let byte = *bytes
            .get(*pos)
            .ok_or_else(|| anyhow!("WASM file ends in the middle of a number"))?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return usize::try_from(value).context("WASM section is too large");
        }
        shift += 7;
        if shift > 35 {
            bail!("WASM file has an invalid length");
        }
    }
}

struct Section {
    start: usize,
    end: usize,
    custom_name: Option<String>,
    payload_start: usize,
}

fn sections(wasm: &[u8]) -> anyhow::Result<Vec<Section>> {
    if wasm.len() < 8 || &wasm[..4] != b"\0asm" {
        bail!("not a WASM file");
    }
    let mut pos = 8;
    let mut found = Vec::new();
    while pos < wasm.len() {
        let start = pos;
        let id = wasm[pos];
        pos += 1;
        let size = read_leb128(wasm, &mut pos)?;
        let end = pos
            .checked_add(size)
            .filter(|end| *end <= wasm.len())
            .ok_or_else(|| anyhow!("WASM section runs past the end of the file"))?;
        let (custom_name, payload_start) = if id == 0 {
            let mut name_pos = pos;
            let name_len = read_leb128(wasm, &mut name_pos)?;
            let name_end = name_pos
                .checked_add(name_len)
                .filter(|name_end| *name_end <= end)
                .ok_or_else(|| anyhow!("WASM custom section name runs past its section"))?;
            let name = std::str::from_utf8(&wasm[name_pos..name_end])
                .context("WASM custom section name is not UTF-8")?;
            (Some(name.to_string()), name_end)
        } else {
            (None, pos)
        };
        found.push(Section {
            start,
            end,
            custom_name,
            payload_start,
        });
        pos = end;
    }
    Ok(found)
}

pub fn read_section(wasm: &[u8]) -> anyhow::Result<String> {
    let mut payload = Vec::new();
    for section in sections(wasm)? {
        if section.custom_name.as_deref() == Some(SECTION) {
            payload.extend_from_slice(&wasm[section.payload_start..section.end]);
        }
    }
    String::from_utf8(payload).context("oxichrome metadata is not UTF-8")
}

pub fn read_wasm(wasm: &[u8]) -> anyhow::Result<ExtensionMetadata> {
    parse_entries(&read_section(wasm)?)
}

pub fn strip_wasm(wasm: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = wasm[..8].to_vec();
    for section in sections(wasm)? {
        if section.custom_name.as_deref() != Some(SECTION) {
            out.extend_from_slice(&wasm[section.start..section.end]);
        }
    }
    Ok(out)
}

pub fn parse_entries(text: &str) -> anyhow::Result<ExtensionMetadata> {
    let mut metadata = ExtensionMetadata::default();
    let mut extensions = Vec::new();
    let mut popups = Vec::new();
    let mut options_pages = Vec::new();
    let mut side_panels: Vec<(String, SidePanel)> = Vec::new();
    let mut omniboxes: Vec<Omnibox> = Vec::new();
    let mut menus = Vec::new();
    let mut commands = Vec::new();
    let mut backgrounds: Vec<(String, String)> = Vec::new();
    let mut scripts: Vec<String> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let entry: Entry = serde_json::from_str(line).with_context(|| {
            format!(
                "unrecognized oxichrome metadata `{line}`; make sure cargo-oxichrome and the oxichrome crate are the same version"
            )
        })?;
        match entry {
            Entry::Extension {
                name,
                display_name,
                default_locale,
                version,
                description,
                gecko_id,
                permissions,
                optional_permissions,
                host_permissions,
                optional_host_permissions,
                chrome_only,
                firefox_only,
                icons,
                action,
                rule_sets,
                web_accessible,
            } => {
                metadata.rule_sets = rule_sets;
                metadata.web_accessible = web_accessible;
                metadata.optional_permissions = optional_permissions;
                metadata.optional_host_permissions = optional_host_permissions;
                metadata.chrome_only = chrome_only;
                metadata.firefox_only = firefox_only;
                extensions.push(name.clone());
                metadata.name = Some(name);
                metadata.display_name = display_name;
                metadata.default_locale = default_locale;
                metadata.version = Some(version);
                metadata.description = description;
                metadata.gecko_id = gecko_id;
                metadata.permissions = permissions;
                metadata.host_permissions = host_permissions;
                metadata.icons = icons;
                metadata.action = action;
            }
            Entry::Background { fn_name, export } => backgrounds.push((fn_name, export)),
            Entry::Message(message) => {
                if !metadata.messages.contains(&message) {
                    metadata.messages.push(message);
                }
            }
            Entry::Event {
                fn_name,
                export,
                namespace,
                event_name,
            } => metadata.event_handlers.push(EventHandler {
                fn_name,
                export,
                namespace,
                event_name,
            }),
            Entry::Popup { fn_name, styles } => {
                metadata.popup_styles = styles;
                popups.push(fn_name)
            }
            Entry::OptionsPage { fn_name, styles } => {
                metadata.options_styles = styles;
                options_pages.push(fn_name)
            }
            Entry::SidePanel { fn_name, panel } => side_panels.push((fn_name, panel)),
            Entry::Omnibox(omnibox) => omniboxes.push(omnibox),
            Entry::ContentScript {
                fn_name,
                export,
                matches,
                run_at,
                all_frames,
                css,
            } => metadata.content_scripts.push(ContentScript {
                fn_name,
                export,
                matches,
                run_at,
                all_frames,
                css,
            }),
            Entry::Menu(menu) => menus.push(menu),
            Entry::Command(command) => commands.push(command),
            Entry::Script {
                name,
                file,
                line,
                column,
            } => scripts.push(format!("`{name}` ({file}:{line}:{column})")),
            Entry::Offscreen(offscreen) => metadata.offscreen.push(offscreen),
        }
    }

    extensions.sort();
    popups.sort();
    options_pages.sort();
    match extensions.len() {
        0 => bail!(
            "no `#[oxichrome::extension]` struct was compiled into this crate; add one, and make sure cargo-oxichrome and the oxichrome crate are the same version"
        ),
        1 => {}
        _ => bail!(
            "found {} `#[oxichrome::extension]` structs ({}); an extension needs exactly one",
            extensions.len(),
            extensions.join(", ")
        ),
    }
    if popups.len() > 1 {
        bail!(
            "found {} `#[oxichrome::popup]` components ({}); an extension can have only one popup",
            popups.len(),
            popups.join(", ")
        );
    }
    if options_pages.len() > 1 {
        bail!(
            "found {} `#[oxichrome::options_page]` components ({}); an extension can have only one options page",
            options_pages.len(),
            options_pages.join(", ")
        );
    }
    scripts.sort();
    check_scripts(&scripts, &metadata)?;
    metadata.offscreen.sort_by(|a, b| a.export.cmp(&b.export));
    check_offscreen(&metadata)?;
    metadata.menus = resolve_menus(menus)?;
    let action_shortcut = metadata
        .action
        .as_ref()
        .and_then(|action| action.shortcut.clone());
    metadata.commands = resolve_commands(commands, action_shortcut.as_ref())?;
    metadata.has_popup = !popups.is_empty();
    metadata.has_options_page = !options_pages.is_empty();
    side_panels.sort_by(|a, b| a.0.cmp(&b.0));
    if side_panels.len() > 1 {
        bail!(
            "found {} `#[oxichrome::side_panel]` components ({}); an extension can have only one side panel",
            side_panels.len(),
            side_panels.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    metadata.side_panel = side_panels.pop().map(|(_, panel)| panel);
    omniboxes.sort_by(|a, b| a.fn_name.cmp(&b.fn_name));
    if omniboxes.len() > 1 {
        bail!(
            "found {} `#[oxichrome::omnibox]` functions ({}); an extension has one address bar keyword",
            omniboxes.len(),
            omniboxes.iter().map(|omnibox| omnibox.fn_name.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    metadata.omnibox = omniboxes.pop();

    backgrounds.sort();
    metadata.background_exports = backgrounds.into_iter().map(|(_, export)| export).collect();
    metadata
        .event_handlers
        .sort_by(|a, b| (&a.fn_name, &a.export).cmp(&(&b.fn_name, &b.export)));
    metadata
        .content_scripts
        .sort_by(|a, b| (&a.fn_name, &a.export).cmp(&(&b.fn_name, &b.export)));
    check_side_panel(&metadata)?;
    check_omnibox(&metadata)?;
    check_event_permissions(&metadata)?;
    check_cookies(&metadata)?;
    Ok(metadata)
}

fn check_event_permission(metadata: &ExtensionMetadata, namespace: &str, permission: &str, variant: &str) -> anyhow::Result<bool> {
    let listed = metadata.permissions.iter().chain(&metadata.optional_permissions).any(|listed| listed == permission);
    if let Some(handler) = metadata.event_handlers.iter().find(|handler| handler.namespace == namespace) {
        if !listed {
            bail!(
                "`{}` listens to `{namespace}::{}`; add `{variant}` to `permissions` in #[oxichrome::extension]",
                handler.fn_name,
                handler.event_name
            );
        }
    }
    Ok(listed)
}

fn check_event_permissions(metadata: &ExtensionMetadata) -> anyhow::Result<()> {
    check_event_permission(metadata, "downloads", "downloads", "Downloads")?;
    check_event_permission(metadata, "idle", "idle", "Idle")?;
    check_event_permission(metadata, "web_navigation", "webNavigation", "WebNavigation")?;
    Ok(())
}

fn check_cookies(metadata: &ExtensionMetadata) -> anyhow::Result<()> {
    let listed = check_event_permission(metadata, "cookies", "cookies", "Cookies")?;
    if listed && metadata.host_permissions.is_empty() && metadata.optional_host_permissions.is_empty() {
        bail!("`Cookies` only reaches sites the extension has host permission for; add them to `host_permissions` in #[oxichrome::extension], like `\"https://example.com/*\"`");
    }
    Ok(())
}

fn check_omnibox(metadata: &ExtensionMetadata) -> anyhow::Result<()> {
    for handler in metadata.event_handlers.iter().filter(|handler| handler.namespace == "omnibox") {
        if handler.event_name == "on_input_changed" {
            bail!(
                "`{}` listens to `omnibox::on_input_changed`; return suggestions from a `#[oxichrome::omnibox(keyword = \"...\")]` function instead, which gets the same text",
                handler.fn_name
            );
        }
        if metadata.omnibox.is_none() {
            bail!(
                "`{}` listens to `omnibox::{}`, but there's no keyword; add a `#[oxichrome::omnibox(keyword = \"...\")]` function",
                handler.fn_name,
                handler.event_name
            );
        }
    }
    Ok(())
}

fn check_side_panel(metadata: &ExtensionMetadata) -> anyhow::Result<()> {
    let panel_events: Vec<&EventHandler> = metadata
        .event_handlers
        .iter()
        .filter(|handler| handler.namespace == "side_panel")
        .collect();
    let Some(panel) = &metadata.side_panel else {
        if let Some(handler) = panel_events.first() {
            bail!(
                "`{}` listens to `side_panel::{}`, but there's no side panel; add a `#[oxichrome::side_panel]` component",
                handler.fn_name,
                handler.event_name
            );
        }
        return Ok(());
    };
    if !panel.open_on_action_click {
        return Ok(());
    }
    if metadata.has_popup {
        bail!(
            "the side panel has `open_on_action_click`, but there's also a `#[oxichrome::popup]`; a toolbar click opens one or the other, so remove one of them (a button in the popup can open the panel with `side_panel::open_on_click`)"
        );
    }
    if let Some(handler) = metadata
        .event_handlers
        .iter()
        .find(|handler| handler.namespace == "action" && handler.event_name == "on_clicked")
    {
        bail!(
            "the side panel has `open_on_action_click`, so `{}` (`action::on_clicked`) would never run in Chrome; remove one of them",
            handler.fn_name
        );
    }
    Ok(())
}

pub fn check_messages(metadata: &ExtensionMetadata, default_keys: Option<&[String]>) -> anyhow::Result<()> {
    let mut used = metadata.messages.clone();
    used.sort_by(|a, b| (&a.at, &a.key).cmp(&(&b.at, &b.key)));
    let Some(first) = used.first() else {
        return Ok(());
    };
    let Some(default) = metadata.default_locale.as_deref() else {
        bail!(
            "`t!({})` ({}) needs `default_locale = \"en\"` (or the language your messages are in) in #[oxichrome::extension]",
            first.key,
            first.at
        );
    };
    let keys = default_keys.unwrap_or_default();
    if let Some(missing) = used.iter().find(|message| !keys.contains(&message.key)) {
        bail!(
            "`t!({})` ({}) isn't in the default locale `{default}`; add it to `static/_locales/{default}/messages.json` (other languages fall back to it, so without it some users would see nothing)",
            missing.key,
            missing.at
        );
    }
    Ok(())
}

fn check_scripts(scripts: &[String], metadata: &ExtensionMetadata) -> anyhow::Result<()> {
    let has = |name: &str| {
        metadata.permissions.iter().chain(&metadata.optional_permissions).any(|permission| permission == name)
    };
    let Some(first) = scripts.first() else {
        return Ok(());
    };
    let listed = if scripts.len() == 1 {
        format!("the on-demand script {first}")
    } else {
        format!("the on-demand scripts {}", scripts.join(", "))
    };
    if !has("scripting") {
        bail!(
            "add `Scripting` to `permissions` in #[oxichrome::extension]; {listed} can't run without it"
        );
    }
    if metadata.host_permissions.is_empty() && metadata.optional_host_permissions.is_empty() && !has("activeTab") {
        bail!(
            "{listed} can't reach any page: add the sites to `host_permissions` in #[oxichrome::extension] (like \"https://example.com/*\"), or add `ActiveTab` to `permissions` to run them in the current tab right after a click on the toolbar button, a menu item or a shortcut"
        );
    }
    Ok(())
}

fn check_offscreen(metadata: &ExtensionMetadata) -> anyhow::Result<()> {
    let has = |name: &str| metadata.permissions.iter().chain(&metadata.optional_permissions).any(|permission| permission == name);
    for offscreen in &metadata.offscreen {
        if offscreen.reasons.iter().any(|reason| reason == "CLIPBOARD") && !has("clipboardWrite") && !has("clipboardRead") {
            bail!(
                "the offscreen function `{}` gives `Clipboard` as a reason; add `ClipboardWrite` (to copy) or `ClipboardRead` (to paste) to `permissions` in #[oxichrome::extension], since both browsers refuse the clipboard without them",
                offscreen.name
            );
        }
    }
    Ok(())
}

fn resolve_menus(mut entries: Vec<MenuEntry>) -> anyhow::Result<Vec<MenuItem>> {
    entries.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

    let mut ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    ids.sort();
    if let Some(pair) = ids.windows(2).find(|pair| pair[0] == pair[1]) {
        let places: Vec<String> = entries
            .iter()
            .filter(|entry| entry.id == pair[0])
            .map(|entry| format!("{}:{}", entry.file, entry.line))
            .collect();
        bail!(
            "two context menu items are named `{}` ({}); menu items need different names because the name is their id",
            pair[0],
            places.join(", ")
        );
    }

    let find = |id: &str| entries.iter().find(|entry| entry.id == id);
    for entry in &entries {
        let mut seen = vec![entry.id.as_str()];
        let mut current = entry;
        while let Some(parent) = &current.parent {
            let Some(next) = find(parent) else {
                bail!(
                    "context menu item `{}` ({}:{}) has parent `{parent}`, which is not a `#[oxichrome::submenu]` in this crate",
                    current.id, current.file, current.line
                );
            };
            if next.item_type != "submenu" {
                bail!(
                    "context menu item `{}` ({}:{}) has parent `{parent}`, which is not a `#[oxichrome::submenu]`",
                    current.id, current.file, current.line
                );
            }
            if seen.contains(&next.id.as_str()) {
                bail!(
                    "context menu submenus `{}` contain each other; a submenu can't be inside itself",
                    seen.join("` -> `")
                );
            }
            seen.push(&next.id);
            current = next;
        }
    }
    for entry in entries.iter().filter(|entry| entry.item_type == "submenu") {
        if !entries
            .iter()
            .any(|child| child.parent.as_deref() == Some(entry.id.as_str()))
        {
            bail!(
                "submenu `{}` ({}:{}) has no items; add items with `parent = {}` or remove it",
                entry.id, entry.file, entry.line, entry.id
            );
        }
    }

    fn contexts_of(entries: &[MenuEntry], entry: &MenuEntry) -> Vec<String> {
        match (&entry.contexts, &entry.parent) {
            (Some(contexts), _) => contexts.clone(),
            (None, Some(parent)) => {
                let parent = entries.iter().find(|other| other.id == *parent).unwrap();
                contexts_of(entries, parent)
            }
            (None, None) => vec!["page".to_string()],
        }
    }

    let top_level_action: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.parent.is_none())
        .filter(|entry| contexts_of(&entries, entry).iter().any(|context| context == "action"))
        .map(|entry| entry.id.as_str())
        .collect();
    if top_level_action.len() > ACTION_MENU_TOP_LEVEL_LIMIT {
        bail!(
            "{} top-level context menu items use the `action` context ({}); browsers show at most {ACTION_MENU_TOP_LEVEL_LIMIT} on the toolbar button, so group some into a submenu",
            top_level_action.len(),
            top_level_action.join(", ")
        );
    }

    let mut ordered = Vec::new();
    fn visit(entries: &[MenuEntry], parent: Option<&str>, ordered: &mut Vec<MenuItem>) {
        for entry in entries.iter().filter(|entry| entry.parent.as_deref() == parent) {
            ordered.push(MenuItem {
                id: entry.id.clone(),
                handler: entry.handler.clone(),
                export: entry.export.clone(),
                item_type: entry.item_type.clone(),
                title: entry.title.clone(),
                contexts: contexts_of(entries, entry),
                checked: entry.checked,
                parent: entry.parent.clone(),
                enabled: entry.enabled,
                visible: entry.visible,
                document_url_patterns: entry.document_url_patterns.clone(),
                target_url_patterns: entry.target_url_patterns.clone(),
            });
            visit(entries, Some(&entry.id), ordered);
        }
    }
    visit(&entries, None, &mut ordered);
    Ok(ordered)
}

pub const ACTION_MENU_TOP_LEVEL_LIMIT: usize = 6;

pub const SHORTCUT_LIMIT: usize = 4;

const SHORTCUT_PLATFORMS: [&str; 4] = ["windows", "mac", "linux", "chromeos"];

fn platform_key(shortcut: &Shortcut, platform: &str) -> Option<String> {
    let key = shortcut.get(platform).or_else(|| shortcut.get("default"))?;
    Some(if platform == "mac" {
        key.split('+')
            .map(|part| if part == "Ctrl" { "Command" } else { part })
            .collect::<Vec<_>>()
            .join("+")
    } else {
        key.clone()
    })
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "mac" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        _ => "ChromeOS",
    }
}

fn resolve_commands(
    mut entries: Vec<CommandEntry>,
    action: Option<&Shortcut>,
) -> anyhow::Result<Vec<Command>> {
    entries.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

    let mut names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    names.sort();
    if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
        let places: Vec<String> = entries
            .iter()
            .filter(|entry| entry.name == pair[0])
            .map(|entry| format!("{}:{}", entry.file, entry.line))
            .collect();
        bail!(
            "two commands are named `{}` ({}); commands need different names because the function name is the command's id",
            pair[0],
            places.join(", ")
        );
    }

    let mut bound: Vec<(String, &Shortcut)> = Vec::new();
    if let Some(shortcut) = action {
        bound.push(("the toolbar button (`action = { shortcut }`)".to_string(), shortcut));
    }
    for entry in &entries {
        if let Some(shortcut) = &entry.shortcut {
            bound.push((format!("`{}` ({}:{})", entry.name, entry.file, entry.line), shortcut));
        }
    }
    if bound.len() > SHORTCUT_LIMIT {
        bail!(
            "{} commands have a shortcut ({}); Chrome rejects an extension that suggests more than {SHORTCUT_LIMIT}, so leave `shortcut` out of the rest and people can assign those themselves",
            bound.len(),
            bound.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    for (index, (name, shortcut)) in bound.iter().enumerate() {
        for (other, earlier) in &bound[..index] {
            let clashes: Vec<(&str, String)> = SHORTCUT_PLATFORMS
                .iter()
                .filter_map(|platform| {
                    let key = platform_key(shortcut, platform)?;
                    (platform_key(earlier, platform).as_ref() == Some(&key)).then_some((*platform, key))
                })
                .collect();
            let Some((_, first_key)) = clashes.first() else {
                continue;
            };
            let place = if clashes.len() == SHORTCUT_PLATFORMS.len()
                && clashes.iter().all(|(platform, key)| *platform == "mac" || key == first_key)
            {
                format!("`{first_key}`")
            } else {
                clashes
                    .iter()
                    .map(|(platform, key)| format!("`{key}` on {}", platform_label(platform)))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            bail!(
                "{other} and {name} both use {place}; the browser gives it to only one of them, so change one"
            );
        }
    }

    Ok(entries
        .into_iter()
        .map(|entry| Command {
            name: entry.name,
            export: entry.export,
            description: entry.description,
            shortcut: entry.shortcut,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTENSION: &str = r#"{"kind":"extension","name":"T","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":null}"#;

    fn section(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut body = vec![name.len() as u8];
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(payload);
        let mut out = vec![0u8];
        let mut size = body.len();
        loop {
            let byte = (size & 0x7f) as u8;
            size >>= 7;
            if size == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
        out.extend(body);
        out
    }

    fn wasm(sections: &[Vec<u8>]) -> Vec<u8> {
        let mut out = b"\0asm\x01\0\0\0".to_vec();
        out.extend_from_slice(&[1, 4, 1, 0x60, 0, 0]);
        for section in sections {
            out.extend_from_slice(section);
        }
        out
    }

    #[test]
    fn reads_every_metadata_section_and_strips_them() {
        let first = format!("{EXTENSION}\n{{\"kind\":\"popup\",\"fn_name\":\"Popup\"}}\n");
        let second = "{\"kind\":\"event\",\"fn_name\":\"b\",\"export\":\"__oxichrome_on_b\",\"namespace\":\"tabs\",\"event_name\":\"on_removed\"}\n{\"kind\":\"event\",\"fn_name\":\"a\",\"export\":\"__oxichrome_on_a\",\"namespace\":\"runtime\",\"event_name\":\"on_installed\"}\n";
        let big = "x".repeat(300);
        let binary = wasm(&[
            section(SECTION, first.as_bytes()),
            section("other", big.as_bytes()),
            section(SECTION, second.as_bytes()),
        ]);
        let metadata = read_wasm(&binary).unwrap();
        assert_eq!(metadata.name.as_deref(), Some("T"));
        assert!(metadata.has_popup);
        let names: Vec<_> = metadata
            .event_handlers
            .iter()
            .map(|handler| handler.fn_name.as_str())
            .collect();
        assert_eq!(names, ["a", "b"]);

        let stripped = strip_wasm(&binary).unwrap();
        assert_eq!(stripped, wasm(&[section("other", big.as_bytes())]));
        assert!(read_wasm(&stripped)
            .unwrap_err()
            .to_string()
            .contains("no `#[oxichrome::extension]`"));
    }

    #[test]
    fn rejects_broken_wasm() {
        assert!(read_wasm(b"not wasm").is_err());
        let mut truncated = wasm(&[section(SECTION, EXTENSION.as_bytes())]);
        truncated.truncate(truncated.len() - 3);
        assert!(read_wasm(&truncated).is_err());
    }

    #[test]
    fn needs_exactly_one_extension_popup_and_options_page() {
        let two = format!("{EXTENSION}\n{}\n", EXTENSION.replace("\"T\"", "\"U\""));
        let err = parse_entries(&two).unwrap_err().to_string();
        assert!(err.contains("found 2 `#[oxichrome::extension]` structs (T, U)"), "{err}");

        let popups = format!(
            "{EXTENSION}\n{{\"kind\":\"popup\",\"fn_name\":\"A\"}}\n{{\"kind\":\"popup\",\"fn_name\":\"B\"}}\n"
        );
        let err = parse_entries(&popups).unwrap_err().to_string();
        assert!(err.contains("only one popup"), "{err}");

        let options = format!(
            "{EXTENSION}\n{{\"kind\":\"options_page\",\"fn_name\":\"A\"}}\n{{\"kind\":\"options_page\",\"fn_name\":\"B\"}}\n"
        );
        let err = parse_entries(&options).unwrap_err().to_string();
        assert!(err.contains("only one options page"), "{err}");
    }

    #[test]
    fn side_panel_checks() {
        let panel = |flag: bool| format!("{{\"kind\":\"side_panel\",\"fn_name\":\"Panel\",\"title\":null,\"open_on_action_click\":{flag}}}");
        let event = |namespace: &str, event: &str| format!("{{\"kind\":\"event\",\"fn_name\":\"f\",\"export\":\"e\",\"namespace\":\"{namespace}\",\"event_name\":\"{event}\"}}");
        let parse = |lines: &[String]| parse_entries(&format!("{EXTENSION}\n{}", lines.join("\n")));
        let ok = parse(&[panel(true)]).unwrap();
        assert_eq!(ok.side_panel, Some(SidePanel { title: None, open_on_action_click: true, styles: Vec::new() }));
        let err = parse(&[panel(false), panel(false).replace("Panel", "Other")]).unwrap_err().to_string();
        assert!(err.contains("only one side panel") && err.contains("Other, Panel"), "{err}");
        let err = parse(&[panel(true), "{\"kind\":\"popup\",\"fn_name\":\"Pop\"}".to_string()]).unwrap_err().to_string();
        assert!(err.contains("#[oxichrome::popup]"), "{err}");
        let err = parse(&[panel(true), event("action", "on_clicked")]).unwrap_err().to_string();
        assert!(err.contains("would never run in Chrome"), "{err}");
        assert!(parse(&[panel(false), event("action", "on_clicked"), "{\"kind\":\"popup\",\"fn_name\":\"Pop\"}".to_string()]).is_ok());
        let err = parse(&[event("side_panel", "on_opened")]).unwrap_err().to_string();
        assert!(err.contains("there's no side panel"), "{err}");
        assert!(parse(&[panel(false), event("side_panel", "on_closed")]).is_ok());
    }

    #[test]
    fn unknown_entries_name_the_version_mismatch() {
        let text = format!("{EXTENSION}\n{{\"kind\":\"teleport\"}}\n");
        let err = format!("{:#}", parse_entries(&text).unwrap_err());
        assert!(err.contains("same version"), "{err}");
    }

    #[test]
    fn reads_icons_action_and_content_scripts() {
        let text = r#"{"kind":"extension","name":"Icons","version":"1.2.3","description":"d","gecko_id":"a@b.c","permissions":["storage"],"host_permissions":["https://example.org/*"],"icons":[[16,"icons/16.png"],[128,"icons/128.png"]],"action":{"default_title":"Hi","default_icon":[[16,"icons/16.png"]]}}
{"kind":"content_script","fn_name":"inject","export":"__oxichrome_cs_inject","matches":["<all_urls>"],"run_at":"document_start","all_frames":true,"css":["a.css"]}
{"kind":"background","fn_name":"start","export":"__oxichrome_bg_start"}
{"kind":"options_page","fn_name":"Options"}"#;
        let metadata = parse_entries(text).unwrap();
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.description.as_deref(), Some("d"));
        assert_eq!(metadata.gecko_id.as_deref(), Some("a@b.c"));
        assert_eq!(metadata.permissions, ["storage"]);
        assert_eq!(metadata.host_permissions, ["https://example.org/*"]);
        assert_eq!(
            metadata.icons,
            vec![(16, "icons/16.png".to_string()), (128, "icons/128.png".to_string())]
        );
        assert_eq!(
            metadata.action,
            Some(ActionMetadata {
                default_title: Some("Hi".to_string()),
                default_icon: Some(ActionIcon::Sizes(vec![(16, "icons/16.png".to_string())])),
                shortcut: None,
            })
        );
        assert_eq!(
            metadata.content_scripts,
            vec![ContentScript {
                fn_name: "inject".to_string(),
                export: "__oxichrome_cs_inject".to_string(),
                matches: vec!["<all_urls>".to_string()],
                run_at: Some("document_start".to_string()),
                all_frames: Some(true),
                css: vec!["a.css".to_string()],
            }]
        );
        assert_eq!(metadata.background_exports, ["__oxichrome_bg_start"]);
        assert!(metadata.has_options_page);
        assert!(!metadata.has_popup);

        let single = r#"{"kind":"extension","name":"A","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":{"default_title":null,"default_icon":"a.png"}}"#;
        assert_eq!(
            parse_entries(single).unwrap().action.unwrap().default_icon,
            Some(ActionIcon::Single("a.png".to_string()))
        );
    }

    fn menu(id: &str, extra: &str, line: usize) -> String {
        format!(
            r#"{{"kind":"menu","id":"{id}","handler":"{id}","item_type":"normal","title":"T","contexts":null,"checked":null,"parent":null,"enabled":null,"visible":null,"document_url_patterns":[],"target_url_patterns":[],"file":"src/lib.rs","line":{line},"column":1{extra}}}"#
        )
    }

    fn with(base: &str, field: &str, value: &str) -> String {
        let marker = format!("\"{field}\":");
        let start = base.find(&marker).unwrap() + marker.len();
        let end = start + base[start..].find(|c| c == ',' || c == '}').unwrap();
        format!("{}{value}{}", &base[..start], &base[end..])
    }

    fn menus(lines: &[String]) -> anyhow::Result<Vec<MenuItem>> {
        Ok(parse_entries(&format!("{EXTENSION}\n{}", lines.join("\n")))?.menus)
    }

    #[test]
    fn menus_come_out_parents_first_in_source_order() {
        let tools = with(&with(&menu("Tools", "", 5), "item_type", "\"submenu\""), "contexts", "[\"page\",\"link\"]");
        let tools = with(&tools, "handler", "null");
        let items = menus(&[
            with(&menu("child_b", "", 9), "parent", "\"Tools\""),
            menu("last", "", 20),
            with(&with(&menu("child_a", "", 7), "parent", "\"Tools\""), "contexts", "[\"link\"]"),
            tools,
            menu("first", "", 1),
        ])
        .unwrap();
        let order: Vec<_> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(order, ["first", "Tools", "child_a", "child_b", "last"]);
        assert_eq!(items[0].contexts, ["page"]);
        assert_eq!(items[2].contexts, ["link"]);
        assert_eq!(items[3].contexts, ["page", "link"]);
    }

    #[test]
    fn menu_mistakes_are_build_errors() {
        let submenu = |id: &str, line| with(&with(&menu(id, "", line), "item_type", "\"submenu\""), "handler", "null");
        let cases: Vec<(Vec<String>, &str)> = vec![
            (vec![menu("same", "", 1), with(&menu("same", "", 2), "file", "\"src/other.rs\"")], "two context menu items are named `same` (src/lib.rs:1, src/other.rs:2)"),
            (vec![submenu("Empty", 1)], "submenu `Empty` (src/lib.rs:1) has no items"),
            (vec![with(&submenu("A", 1), "parent", "\"B\""), with(&submenu("B", 2), "parent", "\"A\""), with(&menu("x", "", 3), "parent", "\"A\"")], "contain each other"),
            (vec![with(&menu("x", "", 1), "parent", "\"y\""), menu("y", "", 2)], "which is not a `#[oxichrome::submenu]`"),
            ((0..7).map(|i| with(&menu(&format!("a{i}"), "", i), "contexts", "[\"action\"]")).collect(), "7 top-level context menu items use the `action` context"),
        ];
        for (lines, expected) in cases {
            let err = menus(&lines).unwrap_err().to_string();
            assert!(err.contains(expected), "{err}");
        }
        let six: Vec<String> = (0..6).map(|i| with(&menu(&format!("a{i}"), "", i), "contexts", "[\"action\"]")).collect();
        assert_eq!(menus(&six).unwrap().len(), 6);
    }

    fn commands(lines: &[String], action: Option<&str>) -> anyhow::Result<Vec<Command>> {
        let action = action.map(|key| format!(r#","action":{{"default_title":null,"default_icon":null,"shortcut":{key}}}"#)).unwrap_or_default();
        let mut text = format!(r#"{{"kind":"extension","name":"A","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[]{action}}}"#);
        if action.is_empty() {
            text = text.replace(r#""icons":[]}"#, r#""icons":[],"action":null}"#);
        }
        for line in lines {
            text.push('\n');
            text.push_str(line);
        }
        parse_entries(&text).map(|metadata| metadata.commands)
    }

    fn command(name: &str, shortcut: &str, line: usize) -> String {
        format!(r#"{{"kind":"command","name":"{name}","export":"__oxichrome_command_{name}_0","description":"d","shortcut":{shortcut},"file":"src/lib.rs","line":{line},"column":1}}"#)
    }

    #[test]
    fn commands_keep_their_shortcuts_in_source_order() {
        let parsed = commands(
            &[
                command("b", r#"{"default":"Alt+Shift+B"}"#, 2),
                command("a", "null", 1),
            ],
            Some(r#"{"default":"Alt+Shift+O"}"#),
        )
        .unwrap();
        assert_eq!(parsed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(parsed[0].shortcut, None);
        assert_eq!(parsed[1].shortcut.as_ref().unwrap()["default"], "Alt+Shift+B");
        assert_eq!(parsed[1].export, "__oxichrome_command_b_0");
    }

    #[test]
    fn command_mistakes_are_build_errors() {
        let key = |k: &str| format!(r#"{{"default":"{k}"}}"#);
        let cases: Vec<(Vec<String>, Option<String>, &str)> = vec![
            (vec![command("go", "null", 1), command("go", "null", 2)], None, "two commands are named `go` (src/lib.rs:1, src/lib.rs:2)"),
            (vec![command("a", &key("Alt+Shift+K"), 1), command("b", &key("Alt+Shift+K"), 2)], None, "`a` (src/lib.rs:1) and `b` (src/lib.rs:2) both use `Alt+Shift+K`;"),
            (vec![command("a", &key("Alt+Shift+K"), 1)], Some(key("Alt+Shift+K")), "the toolbar button (`action = { shortcut }`) and `a` (src/lib.rs:1) both use `Alt+Shift+K`"),
            (vec![command("a", &key("Ctrl+Shift+Y"), 1), command("b", r#"{"default":"Alt+Shift+Y","mac":"Command+Shift+Y"}"#, 2)], None, "both use `Command+Shift+Y` on macOS;"),
            ((1..=5).map(|i| command(&format!("c{i}"), &key(&format!("Alt+Shift+{i}")), i)).collect(), None, "5 commands have a shortcut"),
            ((1..=4).map(|i| command(&format!("c{i}"), &key(&format!("Alt+Shift+{i}")), i)).collect(), Some(key("Alt+Shift+O")), "5 commands have a shortcut (the toolbar button"),
        ];
        for (lines, action, expected) in cases {
            let err = commands(&lines, action.as_deref()).unwrap_err().to_string();
            assert!(err.contains(expected), "{err}");
        }
        let four: Vec<String> = (1..=4).map(|i| command(&format!("c{i}"), &key(&format!("Alt+Shift+{i}")), i)).collect();
        assert_eq!(commands(&four, None).unwrap().len(), 4);
        let unbound: Vec<String> = (1..=9).map(|i| command(&format!("c{i}"), "null", i)).collect();
        assert_eq!(commands(&unbound, Some(&key("Alt+Shift+O"))).unwrap().len(), 9);
    }

    #[test]
    fn on_demand_scripts_need_scripting_and_page_access() {
        let script = |name: &str, line: usize| format!(r#"{{"kind":"script","name":"{name}","file":"src/lib.rs","line":{line},"column":1}}"#);
        let with = |permissions: &str, hosts: &str, lines: &[String]| {
            let ext = format!(r#"{{"kind":"extension","name":"T","version":"1.0.0","description":null,"gecko_id":null,"permissions":{permissions},"host_permissions":{hosts},"icons":[],"action":null}}"#);
            parse_entries(&std::iter::once(ext).chain(lines.iter().cloned()).collect::<Vec<_>>().join("\n"))
        };
        let one = [script("grab", 3)];
        let two = [script("b", 9), script("a", 4)];
        let err = with("[]", r#"["https://a.com/*"]"#, &one).unwrap_err().to_string();
        assert!(err.contains("add `Scripting` to `permissions`") && err.contains("the on-demand script `grab` (src/lib.rs:3:1)"), "{err}");
        let err = with(r#"["scripting"]"#, "[]", &two).unwrap_err().to_string();
        assert!(err.contains("the on-demand scripts `a` (src/lib.rs:4:1), `b` (src/lib.rs:9:1) can't reach any page"), "{err}");
        assert!(with(r#"["scripting"]"#, r#"["https://a.com/*"]"#, &one).is_ok());
        assert!(with(r#"["scripting","activeTab"]"#, "[]", &two).is_ok());
        assert!(with("[]", "[]", &[]).is_ok());
        let optional = |permissions: &str, optional: &str, hosts: &str, optional_hosts: &str| {
            let ext = format!(r#"{{"kind":"extension","name":"T","version":"1.0.0","description":null,"gecko_id":null,"permissions":{permissions},"optional_permissions":{optional},"host_permissions":{hosts},"optional_host_permissions":{optional_hosts},"icons":[],"action":null}}"#);
            parse_entries(&[ext, script("a", 1)].join("\n"))
        };
        assert!(optional("[]", r#"["scripting"]"#, "[]", r#"["https://a.com/*"]"#).is_ok());
        assert!(optional("[]", r#"["scripting","activeTab"]"#, "[]", "[]").is_ok());
        assert!(optional(r#"["scripting"]"#, "[]", "[]", "[]").is_err());
    }
}
