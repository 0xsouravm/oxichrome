pub const EVENTS: &[(&str, &[&str])] = &[
    ("runtime", &["on_installed", "on_message"]),
    ("storage", &["on_changed"]),
    ("tabs", &["on_updated", "on_removed", "on_activated"]),
    ("action", &["on_clicked"]),
    ("alarms", &["on_alarm"]),
    ("notifications", &["on_clicked", "on_closed", "on_button_clicked"]),
    ("context_menus", &["on_clicked"]),
    ("commands", &["on_command"]),
    ("permissions", &["on_added", "on_removed"]),
    ("windows", &["on_created", "on_removed", "on_focus_changed", "on_bounds_changed"]),
    ("side_panel", &["on_opened", "on_closed"]),
    ("omnibox", &["on_input_started", "on_input_changed", "on_input_entered", "on_input_cancelled"]),
    ("downloads", &["on_created", "on_changed", "on_erased"]),
    ("cookies", &["on_changed"]),
    ("idle", &["on_state_changed"]),
    (
        "web_navigation",
        &[
            "on_before_navigate",
            "on_committed",
            "on_dom_content_loaded",
            "on_completed",
            "on_error_occurred",
            "on_history_state_updated",
            "on_reference_fragment_updated",
            "on_created_navigation_target",
            "on_tab_replaced",
        ],
    ),
    ("bookmarks", &["on_created", "on_removed", "on_changed", "on_moved"]),
    ("history", &["on_visited", "on_visit_removed"]),
];

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = (row[j + 1] + 1).min(row[j] + 1).min(previous + usize::from(ca != *cb));
            previous = current;
        }
    }
    row[b.len()]
}

fn closest<'a>(name: &str, options: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(2);
    options
        .map(|option| (distance(name, option), option))
        .filter(|(score, _)| *score <= limit)
        .min_by_key(|(score, _)| *score)
        .map(|(_, option)| option)
}

fn list(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub enum Unknown {
    Namespace(String),
    Event(String),
}

pub fn check(namespace: &str, event: &str) -> Result<(), Unknown> {
    let Some((_, events)) = EVENTS.iter().find(|(name, _)| *name == namespace) else {
        let hint = match closest(namespace, EVENTS.iter().map(|(name, _)| *name)) {
            Some(near) => format!("did you mean `{near}::{event}`? "),
            None => String::new(),
        };
        let names: Vec<&str> = EVENTS.iter().map(|(name, _)| *name).collect();
        return Err(Unknown::Namespace(format!(
            "oxichrome has no events in `{namespace}`; {hint}events come from {}",
            list(&names)
        )));
    };
    if events.contains(&event) {
        return Ok(());
    }
    let hint = match closest(event, events.iter().copied()) {
        Some(near) => format!("did you mean `{namespace}::{near}`? "),
        None => String::new(),
    };
    let what = if events.len() == 1 { "event is" } else { "events are" };
    Err(Unknown::Event(format!(
        "`{namespace}::{event}` isn't an event oxichrome has; {hint}the `{namespace}` {what} {}",
        list(events)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snake(camel: &str) -> String {
        let mut out = String::new();
        for (i, c) in camel.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        }
        out
    }

    #[test]
    fn table_matches_the_core_event_markers() {
        let source = include_str!("../../oxichrome-core/src/event.rs");
        let block = &source[source.find("pub mod events {").unwrap()..];
        let block = &block[..block.find('}').unwrap()];
        let mut markers: Vec<(String, String)> = block
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub struct")?.trim().strip_suffix(';').map(str::to_string))
            .map(|marker| {
                let at = marker.match_indices("On").find(|(i, _)| *i > 0 && marker[i + 2..].starts_with(|c: char| c.is_ascii_uppercase())).unwrap().0;
                (snake(&marker[..at]), format!("on_{}", snake(&marker[at + 2..])))
            })
            .collect();
        let mut table: Vec<(String, String)> = EVENTS.iter().flat_map(|(namespace, events)| events.iter().map(|event| (namespace.to_string(), event.to_string()))).collect();
        markers.sort();
        table.sort();
        assert_eq!(markers, table);
    }

    #[test]
    fn typos_get_a_suggestion() {
        let Err(Unknown::Event(message)) = check("history", "on_visit") else { panic!() };
        assert!(message.contains("did you mean `history::on_visited`?"), "{message}");
        assert!(message.contains("`on_visited` and `on_visit_removed`"), "{message}");
        let Err(Unknown::Namespace(message)) = check("tab", "on_updated") else { panic!() };
        assert!(message.contains("did you mean `tabs::on_updated`?"), "{message}");
        let Err(Unknown::Event(message)) = check("storage", "on_whatever_happens") else { panic!() };
        assert!(!message.contains("did you mean"), "{message}");
        assert!(message.contains("the `storage` event is `on_changed`"), "{message}");
        assert!(check("web_navigation", "on_completed").is_ok());
    }
}
