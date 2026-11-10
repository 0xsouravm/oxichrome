use anyhow::{anyhow, bail, Context};
use serde::Deserialize;

pub const SECTION: &str = "oxichrome.v1";

#[derive(Debug, Default)]
pub struct ExtensionMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub host_permissions: Vec<String>,
    pub background_functions: Vec<String>,
    pub event_handlers: Vec<EventHandler>,
    pub has_popup: bool,
    pub has_options_page: bool,
    pub content_scripts: Vec<ContentScript>,
    pub gecko_id: Option<String>,
    pub icons: Vec<(u32, String)>,
    pub action: Option<ActionMetadata>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ActionMetadata {
    pub default_title: Option<String>,
    pub default_icon: Option<ActionIcon>,
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
    pub namespace: String,
    pub event_name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ContentScript {
    pub fn_name: String,
    pub matches: Vec<String>,
    pub run_at: Option<String>,
    pub all_frames: Option<bool>,
    #[serde(default)]
    pub css: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Entry {
    Extension {
        name: String,
        version: String,
        description: Option<String>,
        gecko_id: Option<String>,
        permissions: Vec<String>,
        host_permissions: Vec<String>,
        icons: Vec<(u32, String)>,
        action: Option<ActionMetadata>,
    },
    Background {
        fn_name: String,
    },
    Event {
        fn_name: String,
        namespace: String,
        event_name: String,
    },
    Popup {
        fn_name: String,
    },
    OptionsPage {
        fn_name: String,
    },
    ContentScript {
        fn_name: String,
        matches: Vec<String>,
        run_at: Option<String>,
        all_frames: Option<bool>,
        css: Vec<String>,
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

pub fn read_wasm(wasm: &[u8]) -> anyhow::Result<ExtensionMetadata> {
    let mut payload = Vec::new();
    for section in sections(wasm)? {
        if section.custom_name.as_deref() == Some(SECTION) {
            payload.extend_from_slice(&wasm[section.payload_start..section.end]);
        }
    }
    let text = std::str::from_utf8(&payload).context("oxichrome metadata is not UTF-8")?;
    parse_entries(text)
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
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let entry: Entry = serde_json::from_str(line).with_context(|| {
            format!(
                "unrecognized oxichrome metadata `{line}`; make sure cargo-oxichrome and the oxichrome crate are the same version"
            )
        })?;
        match entry {
            Entry::Extension {
                name,
                version,
                description,
                gecko_id,
                permissions,
                host_permissions,
                icons,
                action,
            } => {
                extensions.push(name.clone());
                metadata.name = Some(name);
                metadata.version = Some(version);
                metadata.description = description;
                metadata.gecko_id = gecko_id;
                metadata.permissions = permissions;
                metadata.host_permissions = host_permissions;
                metadata.icons = icons;
                metadata.action = action;
            }
            Entry::Background { fn_name } => metadata.background_functions.push(fn_name),
            Entry::Event {
                fn_name,
                namespace,
                event_name,
            } => metadata.event_handlers.push(EventHandler {
                fn_name,
                namespace,
                event_name,
            }),
            Entry::Popup { fn_name } => popups.push(fn_name),
            Entry::OptionsPage { fn_name } => options_pages.push(fn_name),
            Entry::ContentScript {
                fn_name,
                matches,
                run_at,
                all_frames,
                css,
            } => metadata.content_scripts.push(ContentScript {
                fn_name,
                matches,
                run_at,
                all_frames,
                css,
            }),
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
    metadata.has_popup = !popups.is_empty();
    metadata.has_options_page = !options_pages.is_empty();

    metadata.background_functions.sort();
    metadata
        .event_handlers
        .sort_by(|a, b| a.fn_name.cmp(&b.fn_name));
    metadata
        .content_scripts
        .sort_by(|a, b| a.fn_name.cmp(&b.fn_name));
    Ok(metadata)
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
        let second = "{\"kind\":\"event\",\"fn_name\":\"b\",\"namespace\":\"tabs\",\"event_name\":\"on_removed\"}\n{\"kind\":\"event\",\"fn_name\":\"a\",\"namespace\":\"runtime\",\"event_name\":\"on_installed\"}\n";
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
    fn unknown_entries_name_the_version_mismatch() {
        let text = format!("{EXTENSION}\n{{\"kind\":\"teleport\"}}\n");
        let err = format!("{:#}", parse_entries(&text).unwrap_err());
        assert!(err.contains("same version"), "{err}");
    }

    #[test]
    fn reads_icons_action_and_content_scripts() {
        let text = r#"{"kind":"extension","name":"Icons","version":"1.2.3","description":"d","gecko_id":"a@b.c","permissions":["storage"],"host_permissions":["https://example.org/*"],"icons":[[16,"icons/16.png"],[128,"icons/128.png"]],"action":{"default_title":"Hi","default_icon":[[16,"icons/16.png"]]}}
{"kind":"content_script","fn_name":"inject","matches":["<all_urls>"],"run_at":"document_start","all_frames":true,"css":["a.css"]}
{"kind":"background","fn_name":"start"}
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
            })
        );
        assert_eq!(
            metadata.content_scripts,
            vec![ContentScript {
                fn_name: "inject".to_string(),
                matches: vec!["<all_urls>".to_string()],
                run_at: Some("document_start".to_string()),
                all_frames: Some(true),
                css: vec!["a.css".to_string()],
            }]
        );
        assert_eq!(metadata.background_functions, ["start"]);
        assert!(metadata.has_options_page);
        assert!(!metadata.has_popup);

        let single = r#"{"kind":"extension","name":"A","version":"1.0.0","description":null,"gecko_id":null,"permissions":[],"host_permissions":[],"icons":[],"action":{"default_title":null,"default_icon":"a.png"}}"#;
        assert_eq!(
            parse_entries(single).unwrap().action.unwrap().default_icon,
            Some(ActionIcon::Single("a.png".to_string()))
        );
    }
}
