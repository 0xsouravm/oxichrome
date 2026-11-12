use std::result::Result as StdResult;

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;

pub const MAX_DYNAMIC_RULES: usize = 5000;
pub const MAX_SESSION_RULES: usize = 5000;
pub const MAX_REGEX_RULES: usize = 1000;
pub const MAX_RULE_SETS: usize = 100;
pub const MAX_ENABLED_RULE_SETS: usize = 20;
pub const MAX_DISABLED_STATIC_RULES: usize = 5000;
pub const GUARANTEED_STATIC_RULES: usize = 30000;
const MAX_ID: u32 = i32::MAX as u32;
const MAX_REPEAT: u32 = 1000;
const APPENDABLE_REQUEST_HEADERS: &[&str] = &[
    "accept",
    "accept-encoding",
    "accept-language",
    "access-control-request-headers",
    "cache-control",
    "connection",
    "content-language",
    "cookie",
    "forwarded",
    "if-match",
    "if-none-match",
    "keep-alive",
    "range",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "user-agent",
    "via",
    "want-digest",
    "x-forwarded-for",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub id: u32,
    pub priority: u32,
    pub action: Action,
    pub condition: Condition,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Block,
    Allow,
    AllowAllRequests,
    UpgradeScheme,
    Redirect(Redirect),
    ModifyHeaders(Headers),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Redirect {
    Url(String),
    ExtensionPath(String),
    Transform(Transform),
    RegexSubstitution(String),
}

impl Redirect {
    pub fn url(url: impl Into<String>) -> Self {
        Redirect::Url(url.into())
    }

    pub fn extension_path(path: impl Into<String>) -> Self {
        Redirect::ExtensionPath(path.into())
    }

    pub fn regex_substitution(substitution: impl Into<String>) -> Self {
        Redirect::RegexSubstitution(substitution.into())
    }

    pub fn transform(transform: Transform) -> Self {
        Redirect::Transform(transform)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    Remove,
    Set(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParam {
    pub key: String,
    pub value: String,
    pub replace_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    Replace(String),
    Edit { remove: Vec<String>, add_or_replace: Vec<QueryParam> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transform {
    pub scheme: Option<Scheme>,
    pub host: Option<String>,
    pub port: Option<Port>,
    pub path: Option<String>,
    pub query: Option<Query>,
    pub fragment: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Transform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = Some(scheme);
        self
    }

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(Port::Set(port));
        self
    }

    pub fn remove_port(mut self) -> Self {
        self.port = Some(Port::Remove);
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(Query::Replace(query.into()));
        self
    }

    fn edit(&mut self) -> (&mut Vec<String>, &mut Vec<QueryParam>) {
        if !matches!(self.query, Some(Query::Edit { .. })) {
            self.query = Some(Query::Edit { remove: Vec::new(), add_or_replace: Vec::new() });
        }
        match &mut self.query {
            Some(Query::Edit { remove, add_or_replace }) => (remove, add_or_replace),
            _ => unreachable!(),
        }
    }

    pub fn remove_param(mut self, key: impl Into<String>) -> Self {
        self.edit().0.push(key.into());
        self
    }

    pub fn set_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.edit().1.push(QueryParam { key: key.into(), value: value.into(), replace_only: false });
        self
    }

    pub fn replace_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.edit().1.push(QueryParam { key: key.into(), value: value.into(), replace_only: true });
        self
    }

    pub fn fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderOperation {
    Set(String),
    Append(String),
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub operation: HeaderOperation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    pub request: Vec<Header>,
    pub response: Vec<Header>,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(mut self, response: bool, name: impl Into<String>, operation: HeaderOperation) -> Self {
        let header = Header { name: name.into(), operation };
        if response {
            self.response.push(header);
        } else {
            self.request.push(header);
        }
        self
    }

    pub fn set_request(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(false, name, HeaderOperation::Set(value.into()))
    }

    pub fn append_request(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(false, name, HeaderOperation::Append(value.into()))
    }

    pub fn remove_request(self, name: impl Into<String>) -> Self {
        self.push(false, name, HeaderOperation::Remove)
    }

    pub fn set_response(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(true, name, HeaderOperation::Set(value.into()))
    }

    pub fn append_response(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(true, name, HeaderOperation::Append(value.into()))
    }

    pub fn remove_response(self, name: impl Into<String>) -> Self {
        self.push(true, name, HeaderOperation::Remove)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlMatch {
    Filter(String),
    Regex(String),
}

macro_rules! names {
    ($name:ident { $($variant:ident = $text:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),*
                }
            }

            fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some($name::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

names!(ResourceType {
    MainFrame = "main_frame",
    SubFrame = "sub_frame",
    Stylesheet = "stylesheet",
    Script = "script",
    Image = "image",
    Font = "font",
    Object = "object",
    XmlHttpRequest = "xmlhttprequest",
    Ping = "ping",
    CspReport = "csp_report",
    Media = "media",
    WebSocket = "websocket",
    Other = "other",
});

names!(Method {
    Connect = "connect",
    Delete = "delete",
    Get = "get",
    Head = "head",
    Options = "options",
    Patch = "patch",
    Post = "post",
    Put = "put",
    Other = "other",
});

names!(DomainType {
    FirstParty = "firstParty",
    ThirdParty = "thirdParty",
});

const CHROME_ONLY_TYPES: &[&str] = &["webtransport", "webbundle"];
const FIREFOX_ONLY_TYPES: &[&str] = &["beacon", "imageset", "xml_dtd", "xslt", "speculative", "object_subrequest", "web_manifest"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Condition {
    pub url: Option<UrlMatch>,
    pub case_sensitive: bool,
    pub request_domains: Option<Vec<String>>,
    pub excluded_request_domains: Vec<String>,
    pub initiator_domains: Option<Vec<String>>,
    pub excluded_initiator_domains: Vec<String>,
    pub resource_types: Option<Vec<ResourceType>>,
    pub excluded_resource_types: Vec<ResourceType>,
    pub methods: Option<Vec<Method>>,
    pub excluded_methods: Vec<Method>,
    pub domain_type: Option<DomainType>,
    pub tab_ids: Option<Vec<i32>>,
    pub excluded_tab_ids: Vec<i32>,
}

fn strings<I: IntoIterator<Item = S>, S: Into<String>>(items: I) -> Vec<String> {
    items.into_iter().map(Into::into).collect()
}

impl Rule {
    fn new(id: u32, action: Action) -> Self {
        Rule { id, priority: 1, action, condition: Condition::default() }
    }

    pub fn block(id: u32) -> Self {
        Rule::new(id, Action::Block)
    }

    pub fn allow(id: u32) -> Self {
        Rule::new(id, Action::Allow)
    }

    pub fn allow_all_requests(id: u32) -> Self {
        Rule::new(id, Action::AllowAllRequests)
    }

    pub fn upgrade_scheme(id: u32) -> Self {
        Rule::new(id, Action::UpgradeScheme)
    }

    pub fn redirect(id: u32, redirect: Redirect) -> Self {
        Rule::new(id, Action::Redirect(redirect))
    }

    pub fn modify_headers(id: u32, headers: Headers) -> Self {
        Rule::new(id, Action::ModifyHeaders(headers))
    }

    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn url_filter(mut self, filter: impl Into<String>) -> Self {
        self.condition.url = Some(UrlMatch::Filter(filter.into()));
        self
    }

    pub fn regex(mut self, regex: impl Into<String>) -> Self {
        self.condition.url = Some(UrlMatch::Regex(regex.into()));
        self
    }

    pub fn case_sensitive(mut self) -> Self {
        self.condition.case_sensitive = true;
        self
    }

    pub fn request_domains<I: IntoIterator<Item = S>, S: Into<String>>(mut self, domains: I) -> Self {
        self.condition.request_domains = Some(strings(domains));
        self
    }

    pub fn excluded_request_domains<I: IntoIterator<Item = S>, S: Into<String>>(mut self, domains: I) -> Self {
        self.condition.excluded_request_domains = strings(domains);
        self
    }

    pub fn initiator_domains<I: IntoIterator<Item = S>, S: Into<String>>(mut self, domains: I) -> Self {
        self.condition.initiator_domains = Some(strings(domains));
        self
    }

    pub fn excluded_initiator_domains<I: IntoIterator<Item = S>, S: Into<String>>(mut self, domains: I) -> Self {
        self.condition.excluded_initiator_domains = strings(domains);
        self
    }

    pub fn resource_types(mut self, types: impl IntoIterator<Item = ResourceType>) -> Self {
        self.condition.resource_types = Some(types.into_iter().collect());
        self
    }

    pub fn excluded_resource_types(mut self, types: impl IntoIterator<Item = ResourceType>) -> Self {
        self.condition.excluded_resource_types = types.into_iter().collect();
        self
    }

    pub fn methods(mut self, methods: impl IntoIterator<Item = Method>) -> Self {
        self.condition.methods = Some(methods.into_iter().collect());
        self
    }

    pub fn excluded_methods(mut self, methods: impl IntoIterator<Item = Method>) -> Self {
        self.condition.excluded_methods = methods.into_iter().collect();
        self
    }

    pub fn first_party(mut self) -> Self {
        self.condition.domain_type = Some(DomainType::FirstParty);
        self
    }

    pub fn third_party(mut self) -> Self {
        self.condition.domain_type = Some(DomainType::ThirdParty);
        self
    }

    pub fn tab_ids(mut self, tabs: impl IntoIterator<Item = i32>) -> Self {
        self.condition.tab_ids = Some(tabs.into_iter().collect());
        self
    }

    pub fn excluded_tab_ids(mut self, tabs: impl IntoIterator<Item = i32>) -> Self {
        self.condition.excluded_tab_ids = tabs.into_iter().collect();
        self
    }

    fn needs_host_access(&self) -> bool {
        matches!(self.action, Action::Redirect(_) | Action::ModifyHeaders(_))
    }

    fn is_regex(&self) -> bool {
        matches!(self.condition.url, Some(UrlMatch::Regex(_)))
    }

    fn extension_path(&self) -> Option<&str> {
        match &self.action {
            Action::Redirect(Redirect::ExtensionPath(path)) => path.split(['?', '#']).next(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Static,
    Dynamic,
    Session,
}

impl Scope {
    fn name(self) -> &'static str {
        match self {
            Scope::Static => "static",
            Scope::Dynamic => "dynamic",
            Scope::Session => "session",
        }
    }
}

fn ascii(text: &str, what: &str) -> StdResult<(), String> {
    if text.is_ascii() {
        Ok(())
    } else {
        Err(format!("{what} `{text}` has characters outside ASCII, which both browsers refuse; percent-encode them (and write domains in punycode, like `xn--mnchen-3ya.de`)"))
    }
}

pub fn check_domain(domain: &str) -> StdResult<(), String> {
    if domain.is_empty() {
        return Err("a domain can't be empty".to_string());
    }
    ascii(domain, "the domain")?;
    if domain.contains("://") {
        return Err(format!("`{domain}` is a URL; write only the domain, like `example.com`"));
    }
    if domain.starts_with('[') && domain.ends_with(']') {
        let inner = &domain[1..domain.len() - 1];
        if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.') {
            return Ok(());
        }
        return Err(format!("`{domain}` isn't an IPv6 address"));
    }
    if domain.contains('*') {
        return Err(format!("`{domain}` has a `*`; domains already cover their subdomains, so write `{}`", domain.trim_start_matches("*.").trim_start_matches('*')));
    }
    if domain.contains(':') || domain.contains('/') {
        return Err(format!("`{domain}` has a port or a path; write only the domain, like `example.com`"));
    }
    if domain.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(format!("`{domain}` has capital letters, which Firefox refuses; write `{}`", domain.to_ascii_lowercase()));
    }
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
        return Err(format!("`{domain}` has an empty part between dots"));
    }
    if let Some(bad) = domain.chars().find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == '_')) {
        return Err(format!("`{domain}` has `{bad}`, which can't be in a domain"));
    }
    if domain.split('.').any(|label| label.len() > 63) {
        return Err(format!("`{domain}` has a part longer than 63 characters"));
    }
    Ok(())
}

fn check_header_name(name: &str) -> StdResult<(), String> {
    if name.is_empty() {
        return Err("a header name can't be empty".to_string());
    }
    if let Some(bad) = name.chars().find(|c| !(c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(*c))) {
        return Err(format!("the header name `{name}` has `{}`, which header names can't have", bad.escape_default()));
    }
    Ok(())
}

fn check_header_value(name: &str, value: &str) -> StdResult<(), String> {
    if value.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
        return Err(format!("the value for header `{name}` has a line break or NUL, which Chrome refuses"));
    }
    Ok(())
}

pub fn check_regex(regex: &str) -> StdResult<usize, String> {
    Regex::check(regex)
}

struct Regex<'a> {
    text: &'a [u8],
    at: usize,
    groups: usize,
    depth: usize,
}

const MAX_NESTING: usize = 200;

impl<'a> Regex<'a> {
    fn check(regex: &'a str) -> StdResult<usize, String> {
        if regex.is_empty() {
            return Err("the regex can't be empty".to_string());
        }
        ascii(regex, "the regex")?;
        let mut parser = Regex { text: regex.as_bytes(), at: 0, groups: 0, depth: 0 };
        parser.alternation()?;
        if parser.at < parser.text.len() {
            return Err(format!("the regex `{regex}` has a `)` without a matching `(`"));
        }
        Ok(parser.groups)
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn fail(&self, why: &str) -> StdResult<(), String> {
        Err(format!("the regex `{}` {why}", String::from_utf8_lossy(self.text)))
    }

    fn alternation(&mut self) -> StdResult<u32, String> {
        let mut most = 1;
        loop {
            most = most.max(self.sequence()?);
            if self.peek() == Some(b'|') {
                self.at += 1;
            } else {
                return Ok(most);
            }
        }
    }

    fn sequence(&mut self) -> StdResult<u32, String> {
        let mut most = 1;
        while let Some(c) = self.peek() {
            if c == b'|' || c == b')' {
                break;
            }
            let weight = self.atom()?;
            most = most.max(self.quantifier(weight)?);
        }
        Ok(most)
    }

    fn atom(&mut self) -> StdResult<u32, String> {
        let c = self.peek().unwrap_or(0);
        match c {
            b'(' => {
                self.at += 1;
                if self.peek() == Some(b'?') {
                    let rest = &self.text[self.at..];
                    if rest.starts_with(b"?:") {
                        self.at += 2;
                    } else if rest.starts_with(b"?=") || rest.starts_with(b"?!") || rest.starts_with(b"?<=") || rest.starts_with(b"?<!") {
                        self.fail("looks ahead or behind, which Chrome's regex engine can't do")?;
                    } else if rest.starts_with(b"?<") || rest.starts_with(b"?P<") {
                        self.fail("names a group; use a plain `(...)` group and refer to it by number")?;
                    } else {
                        self.fail("sets flags inside the regex like `(?i)`, which Firefox can't read; rules ignore case unless they ask for `case_sensitive`")?;
                    }
                } else {
                    self.groups += 1;
                }
                self.depth += 1;
                if self.depth > MAX_NESTING {
                    self.fail(&format!("nests groups more than {MAX_NESTING} deep"))?;
                }
                let weight = self.alternation()?;
                self.depth -= 1;
                if self.peek() != Some(b')') {
                    self.fail("has a `(` without a matching `)`")?;
                }
                self.at += 1;
                Ok(weight)
            }
            b'[' => {
                self.class()?;
                Ok(1)
            }
            b'\\' => {
                self.escape(false)?;
                Ok(1)
            }
            b'*' | b'+' | b'?' => {
                self.fail(&format!("has `{}` with nothing before it to repeat", c as char))?;
                Ok(1)
            }
            b'{' => {
                self.fail("has a `{` that doesn't start a repeat like `{2,5}`; write `\\{` for the character")?;
                Ok(1)
            }
            b'}' => {
                self.fail("has a `}` without a `{`; write `\\}` for the character")?;
                Ok(1)
            }
            b']' => {
                self.fail("has a `]` without a `[`; write `\\]` for the character")?;
                Ok(1)
            }
            _ => {
                self.at += 1;
                Ok(1)
            }
        }
    }

    fn escape(&mut self, in_class: bool) -> StdResult<(), String> {
        self.at += 1;
        let Some(c) = self.peek() else {
            return self.fail("ends with a lone `\\`");
        };
        self.at += 1;
        match c {
            b'd' | b'D' | b'w' | b'W' | b's' | b'S' | b't' | b'n' | b'r' | b'f' | b'v' => Ok(()),
            b'b' | b'B' if !in_class => Ok(()),
            b'x' => {
                let hex = self.text.get(self.at..self.at + 2).filter(|pair| pair.iter().all(u8::is_ascii_hexdigit));
                if hex.is_none() {
                    return self.fail("has `\\x` without two hex digits after it");
                }
                self.at += 2;
                Ok(())
            }
            b'1'..=b'9' => self.fail("refers back to a group like `\\1`, which Chrome's regex engine can't do"),
            b'0' => self.fail("has `\\0`, which the browsers read differently; write `\\x00`"),
            c if c.is_ascii_alphanumeric() => self.fail(&format!("has `\\{}`, which one of the browsers doesn't understand", c as char)),
            _ => Ok(()),
        }
    }

    fn class(&mut self) -> StdResult<(), String> {
        self.at += 1;
        if self.peek() == Some(b'^') {
            self.at += 1;
        }
        if self.text[self.at..].starts_with(b"[:") {
            return self.fail("uses a class like `[[:alpha:]]`, which Firefox can't read; write `[a-zA-Z]`");
        }
        let mut first = true;
        loop {
            match self.peek() {
                None => return self.fail("has a `[` without a matching `]`"),
                Some(b']') if !first => {
                    self.at += 1;
                    return Ok(());
                }
                Some(b']') => return self.fail("has an empty `[]`, which the browsers read differently"),
                Some(b'[') => return self.fail("has a `[` inside `[...]`; write `\\[` for the character"),
                Some(b'\\') => self.escape(true)?,
                Some(_) => self.at += 1,
            }
            first = false;
        }
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        if self.at == start {
            return None;
        }
        std::str::from_utf8(&self.text[start..self.at]).ok()?.parse().ok().or(Some(u32::MAX))
    }

    fn quantifier(&mut self, weight: u32) -> StdResult<u32, String> {
        let count = match self.peek() {
            Some(b'*') | Some(b'+') | Some(b'?') => {
                self.at += 1;
                1
            }
            Some(b'{') => {
                self.at += 1;
                let low = self.number();
                let Some(low) = low else {
                    self.fail("has a `{` that doesn't start a repeat like `{2,5}`; write `\\{` for the character")?;
                    return Ok(weight);
                };
                let high = if self.peek() == Some(b',') {
                    self.at += 1;
                    self.number()
                } else {
                    Some(low)
                };
                if self.peek() != Some(b'}') {
                    self.fail("has a repeat that isn't closed with `}`")?;
                }
                self.at += 1;
                if let Some(high) = high {
                    if high < low {
                        self.fail(&format!("repeats `{{{low},{high}}}` with the larger number first"))?;
                    }
                }
                let most = high.unwrap_or(low).max(low);
                if most > MAX_REPEAT {
                    self.fail(&format!("repeats something {most} times; Chrome allows at most {MAX_REPEAT}"))?;
                }
                most.max(1)
            }
            _ => return Ok(weight),
        };
        if matches!(self.peek(), Some(b'*') | Some(b'+') | Some(b'{')) {
            self.fail("repeats a repeat (like `a**` or `a{2}{3}`); wrap the inner part in `(?:...)`")?;
        }
        if self.peek() == Some(b'?') {
            self.at += 1;
        }
        let total = weight.saturating_mul(count);
        if total > MAX_REPEAT {
            self.fail(&format!("repeats inside a repeat {total} times in all; Chrome allows at most {MAX_REPEAT}"))?;
        }
        Ok(total)
    }
}

fn check_substitution(substitution: &str, groups: usize) -> StdResult<(), String> {
    ascii(substitution, "the regex substitution")?;
    let bytes = substitution.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'\\' {
            match bytes.get(at + 1) {
                Some(d) if d.is_ascii_digit() => {
                    let n = (d - b'0') as usize;
                    if n > groups {
                        return Err(format!("the regex substitution uses `\\{n}`, but the regex has {groups} group{}", if groups == 1 { "" } else { "s" }));
                    }
                }
                Some(b'\\') => {}
                _ => return Err("the regex substitution has a `\\` that isn't followed by a group number or another `\\`".to_string()),
            }
            at += 2;
        } else {
            at += 1;
        }
    }
    Ok(())
}

fn check_url(url: &str) -> StdResult<(), String> {
    ascii(url, "the redirect URL")?;
    let scheme = url.split(':').next().unwrap_or("");
    let has_scheme = url.contains(':') && !scheme.is_empty() && scheme.starts_with(|c: char| c.is_ascii_alphabetic()) && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    if !has_scheme {
        return Err(format!("the redirect URL `{url}` isn't a full URL; write it with a scheme, like `https://example.com/`"));
    }
    if scheme.eq_ignore_ascii_case("javascript") {
        return Err("rules can't redirect to `javascript:` URLs".to_string());
    }
    if url.chars().any(|c| c.is_ascii_whitespace() || c.is_ascii_control()) {
        return Err(format!("the redirect URL `{url}` has spaces or control characters; percent-encode them"));
    }
    Ok(())
}

fn check_types(rule: &Rule) -> StdResult<(), String> {
    let c = &rule.condition;
    for (empty, what) in [
        (c.request_domains.as_ref().is_some_and(Vec::is_empty), "request_domains"),
        (c.initiator_domains.as_ref().is_some_and(Vec::is_empty), "initiator_domains"),
        (c.resource_types.as_ref().is_some_and(Vec::is_empty), "resource_types"),
        (c.methods.as_ref().is_some_and(Vec::is_empty), "methods"),
        (c.tab_ids.as_ref().is_some_and(Vec::is_empty), "tab_ids"),
    ] {
        if empty {
            return Err(format!("`{what}` is an empty list, which would match nothing and which both browsers refuse; leave it out to match everything"));
        }
    }
    if let Some(both) = c.resource_types.iter().flatten().find(|t| c.excluded_resource_types.contains(t)) {
        return Err(format!("`{}` is both in resource_types and excluded_resource_types", both.as_str()));
    }
    if let Some(both) = c.methods.iter().flatten().find(|m| c.excluded_methods.contains(m)) {
        return Err(format!("`{}` is both in methods and excluded_methods", both.as_str()));
    }
    if rule.action == Action::AllowAllRequests {
        if c.resource_types.is_none() {
            return Err("an allow_all_requests rule needs resource_types with `MainFrame`, `SubFrame` or both".to_string());
        }
        if let Some(other) = c.resource_types.iter().flatten().find(|t| !matches!(t, ResourceType::MainFrame | ResourceType::SubFrame)) {
            return Err(format!("an allow_all_requests rule can only list `MainFrame` and `SubFrame` in resource_types, not `{other:?}`"));
        }
    }
    Ok(())
}

impl Rule {
    pub fn check(&self, scope: Scope) -> StdResult<(), String> {
        self.check_inner(scope).map_err(|why| format!("rule {}: {why}", self.id))
    }

    fn check_inner(&self, scope: Scope) -> StdResult<(), String> {
        if self.id == 0 || self.id > MAX_ID {
            return Err(format!("the id must be from 1 to {MAX_ID}"));
        }
        if self.priority == 0 || self.priority > MAX_ID {
            return Err(format!("the priority must be from 1 to {MAX_ID}"));
        }
        let c = &self.condition;
        let mut groups = 0;
        match &c.url {
            Some(UrlMatch::Filter(filter)) => {
                if filter.is_empty() {
                    return Err("the URL filter can't be empty; leave it out to match every URL".to_string());
                }
                ascii(filter, "the URL filter")?;
                if filter.starts_with("||*") {
                    return Err(format!("the URL filter `{filter}` starts with `||*`, which both browsers refuse; drop the `*`"));
                }
            }
            Some(UrlMatch::Regex(regex)) => groups = check_regex(regex)?,
            None => {}
        }
        for (list, what) in [
            (c.request_domains.as_deref().unwrap_or_default(), "request_domains"),
            (&c.excluded_request_domains[..], "excluded_request_domains"),
            (c.initiator_domains.as_deref().unwrap_or_default(), "initiator_domains"),
            (&c.excluded_initiator_domains[..], "excluded_initiator_domains"),
        ] {
            for domain in list {
                check_domain(domain).map_err(|why| format!("{what}: {why}"))?;
            }
        }
        check_types(self)?;
        if (c.tab_ids.is_some() || !c.excluded_tab_ids.is_empty()) && scope != Scope::Session {
            return Err(format!("tab_ids and excluded_tab_ids only work in session rules, not in {} rules", scope.name()));
        }
        match &self.action {
            Action::Redirect(redirect) => match redirect {
                Redirect::Url(url) => check_url(url)?,
                Redirect::ExtensionPath(path) => {
                    if !path.starts_with('/') {
                        return Err(format!("the extension path `{path}` must start with `/`, like `/blocked.html`"));
                    }
                    ascii(path, "the extension path")?;
                    let file = path.split(['?', '#']).next().unwrap_or("");
                    if file == "/" || file.ends_with('/') {
                        return Err(format!("the extension path `{path}` must name a file, like `/blocked.html`"));
                    }
                    if file.split('/').skip(1).any(|part| part.is_empty() || part == "." || part == "..") {
                        return Err(format!("the extension path `{path}` has an empty, `.` or `..` part"));
                    }
                    if file.starts_with("/wasm/") {
                        return Err(format!("the extension path `{path}` points at the extension's own code"));
                    }
                }
                Redirect::RegexSubstitution(substitution) => {
                    if !matches!(c.url, Some(UrlMatch::Regex(_))) {
                        return Err("a regex substitution needs the rule to match with `regex(...)`".to_string());
                    }
                    check_substitution(substitution, groups)?;
                }
                Redirect::Transform(transform) => check_transform(transform)?,
            },
            Action::ModifyHeaders(headers) => {
                if headers.request.is_empty() && headers.response.is_empty() {
                    return Err("a modify_headers rule needs at least one header change".to_string());
                }
                for (list, response) in [(&headers.request, false), (&headers.response, true)] {
                    for header in list {
                        check_header_name(&header.name)?;
                        match &header.operation {
                            HeaderOperation::Set(value) => check_header_value(&header.name, value)?,
                            HeaderOperation::Append(value) => {
                                check_header_value(&header.name, value)?;
                                if !response && !APPENDABLE_REQUEST_HEADERS.contains(&header.name.to_ascii_lowercase().as_str()) {
                                    return Err(format!(
                                        "Chrome only appends to standard request headers that take several values (like `accept` or `cookie`), not `{}`; use set_request instead",
                                        header.name
                                    ));
                                }
                            }
                            HeaderOperation::Remove => {}
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn check_transform(transform: &Transform) -> StdResult<(), String> {
    if transform == &Transform::default() {
        return Err("the redirect transform doesn't change anything".to_string());
    }
    if let Some(host) = &transform.host {
        check_domain(host).map_err(|why| format!("the transform host: {why}"))?;
    }
    if let Some(path) = &transform.path {
        ascii(path, "the transform path")?;
        if !path.is_empty() && !path.starts_with('/') {
            return Err(format!("the transform path `{path}` must start with `/` (or be empty to clear it)"));
        }
    }
    match &transform.query {
        Some(Query::Replace(query)) => {
            ascii(query, "the transform query")?;
            if !query.is_empty() && !query.starts_with('?') {
                return Err(format!("the transform query `{query}` must start with `?` (or be empty to clear it)"));
            }
        }
        Some(Query::Edit { remove, add_or_replace }) => {
            for key in remove.iter().chain(add_or_replace.iter().map(|param| &param.key)) {
                if key.is_empty() {
                    return Err("a query parameter name can't be empty".to_string());
                }
                ascii(key, "the query parameter name")?;
            }
            for param in add_or_replace {
                ascii(&param.value, "the query parameter value")?;
            }
        }
        None => {}
    }
    for (part, what) in [(&transform.username, "the transform username"), (&transform.password, "the transform password")] {
        if let Some(part) = part {
            ascii(part, what)?;
        }
    }
    if let Some(fragment) = &transform.fragment {
        ascii(fragment, "the transform fragment")?;
        if !fragment.is_empty() && !fragment.starts_with('#') {
            return Err(format!("the transform fragment `{fragment}` must start with `#` (or be empty to clear it)"));
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawRule {
    id: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<f64>,
    action: Option<RawAction>,
    condition: Option<RawCondition>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawAction {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect: Option<RawRedirect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_headers: Option<Vec<RawHeader>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_headers: Option<Vec<RawHeader>>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawRedirect {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extension_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<RawTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regex_substitution: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawTransform {
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query_transform: Option<RawQueryTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fragment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawQueryTransform {
    #[serde(skip_serializing_if = "Option::is_none")]
    remove_params: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    add_or_replace_params: Option<Vec<RawParam>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawParam {
    key: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replace_only: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct RawHeader {
    header: Option<String>,
    operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    url_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regex_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_url_filter_case_sensitive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_request_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initiator_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_initiator_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_resource_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_methods: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_request_methods: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_ids: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_tab_ids: Option<Vec<i32>>,
}

fn opt_list(list: &[String]) -> Option<Vec<String>> {
    (!list.is_empty()).then(|| list.to_vec())
}

fn opt_names<T: Copy>(list: &[T], name: fn(T) -> &'static str) -> Option<Vec<String>> {
    (!list.is_empty()).then(|| list.iter().map(|item| name(*item).to_string()).collect())
}

fn raw_headers(list: &[Header]) -> Option<Vec<RawHeader>> {
    (!list.is_empty()).then(|| {
        list.iter()
            .map(|header| {
                let (operation, value) = match &header.operation {
                    HeaderOperation::Set(value) => ("set", Some(value.clone())),
                    HeaderOperation::Append(value) => ("append", Some(value.clone())),
                    HeaderOperation::Remove => ("remove", None),
                };
                RawHeader { header: Some(header.name.clone()), operation: Some(operation.to_string()), value }
            })
            .collect()
    })
}

impl From<&Rule> for RawRule {
    fn from(rule: &Rule) -> Self {
        let mut action = RawAction::default();
        action.kind = Some(
            match &rule.action {
                Action::Block => "block",
                Action::Allow => "allow",
                Action::AllowAllRequests => "allowAllRequests",
                Action::UpgradeScheme => "upgradeScheme",
                Action::Redirect(_) => "redirect",
                Action::ModifyHeaders(_) => "modifyHeaders",
            }
            .to_string(),
        );
        match &rule.action {
            Action::Redirect(redirect) => {
                let mut raw = RawRedirect::default();
                match redirect {
                    Redirect::Url(url) => raw.url = Some(url.clone()),
                    Redirect::ExtensionPath(path) => raw.extension_path = Some(path.clone()),
                    Redirect::RegexSubstitution(substitution) => raw.regex_substitution = Some(substitution.clone()),
                    Redirect::Transform(t) => {
                        let (query, query_transform) = match &t.query {
                            None => (None, None),
                            Some(Query::Replace(query)) => (Some(query.clone()), None),
                            Some(Query::Edit { remove, add_or_replace }) => (
                                None,
                                Some(RawQueryTransform {
                                    remove_params: opt_list(remove),
                                    add_or_replace_params: (!add_or_replace.is_empty()).then(|| {
                                        add_or_replace
                                            .iter()
                                            .map(|p| RawParam { key: p.key.clone(), value: p.value.clone(), replace_only: p.replace_only.then_some(true) })
                                            .collect()
                                    }),
                                }),
                            ),
                        };
                        raw.transform = Some(RawTransform {
                            scheme: t.scheme.map(|s| if s == Scheme::Http { "http" } else { "https" }.to_string()),
                            host: t.host.clone(),
                            port: t.port.map(|p| match p {
                                Port::Remove => String::new(),
                                Port::Set(port) => port.to_string(),
                            }),
                            path: t.path.clone(),
                            query,
                            query_transform,
                            fragment: t.fragment.clone(),
                            username: t.username.clone(),
                            password: t.password.clone(),
                        });
                    }
                }
                action.redirect = Some(raw);
            }
            Action::ModifyHeaders(headers) => {
                action.request_headers = raw_headers(&headers.request);
                action.response_headers = raw_headers(&headers.response);
            }
            _ => {}
        }
        let c = &rule.condition;
        let (url_filter, regex_filter) = match &c.url {
            Some(UrlMatch::Filter(filter)) => (Some(filter.clone()), None),
            Some(UrlMatch::Regex(regex)) => (None, Some(regex.clone())),
            None => (None, None),
        };
        RawRule {
            id: Some(rule.id as f64),
            priority: Some(rule.priority as f64),
            action: Some(action),
            condition: Some(RawCondition {
                url_filter,
                regex_filter,
                is_url_filter_case_sensitive: c.case_sensitive.then_some(true),
                request_domains: c.request_domains.clone(),
                excluded_request_domains: opt_list(&c.excluded_request_domains),
                initiator_domains: c.initiator_domains.clone(),
                excluded_initiator_domains: opt_list(&c.excluded_initiator_domains),
                resource_types: c.resource_types.as_ref().map(|list| list.iter().map(|t| t.as_str().to_string()).collect()),
                excluded_resource_types: opt_names(&c.excluded_resource_types, ResourceType::as_str),
                request_methods: c.methods.as_ref().map(|list| list.iter().map(|m| m.as_str().to_string()).collect()),
                excluded_request_methods: opt_names(&c.excluded_methods, Method::as_str),
                domain_type: c.domain_type.map(|d| d.as_str().to_string()),
                tab_ids: c.tab_ids.clone(),
                excluded_tab_ids: (!c.excluded_tab_ids.is_empty()).then(|| c.excluded_tab_ids.clone()),
            }),
        }
    }
}

fn whole(value: Option<f64>, what: &str) -> StdResult<u32, String> {
    match value {
        Some(n) if n.fract() == 0.0 && n >= 0.0 && n <= u32::MAX as f64 => Ok(n as u32),
        Some(n) => Err(format!("the {what} {n} isn't a whole number from 1 to {MAX_ID}")),
        None => Err(format!("the rule has no {what}")),
    }
}

fn parse_list<T>(list: Option<Vec<String>>, what: &str, parse: fn(&str) -> Option<T>, hint: fn(&str) -> String) -> StdResult<Vec<T>, String> {
    let list = list.unwrap_or_default();
    if list.is_empty() {
        return Ok(Vec::new());
    }
    list.iter().map(|item| parse(item).ok_or_else(|| format!("{what}: {}", hint(item)))).collect()
}

fn type_hint(item: &str) -> String {
    if CHROME_ONLY_TYPES.contains(&item) {
        format!("`{item}` is a resource type only Chrome has")
    } else if FIREFOX_ONLY_TYPES.contains(&item) {
        format!("`{item}` is a resource type only Firefox has")
    } else {
        format!("`{item}` isn't a resource type; the types are {}", ResourceType::ALL.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", "))
    }
}

fn method_hint(item: &str) -> String {
    if Method::parse(&item.to_ascii_lowercase()).is_some() {
        format!("write the method `{item}` in lower case, `{}`", item.to_ascii_lowercase())
    } else {
        format!("`{item}` isn't a request method; the methods are {}", Method::ALL.iter().map(|m| m.as_str()).collect::<Vec<_>>().join(", "))
    }
}

fn empty_list(list: &Option<Vec<String>>, what: &str) -> StdResult<(), String> {
    if list.as_ref().is_some_and(Vec::is_empty) {
        return Err(format!("`{what}` is an empty list, which both browsers refuse; leave it out instead"));
    }
    Ok(())
}

impl TryFrom<RawRule> for Rule {
    type Error = String;

    fn try_from(raw: RawRule) -> StdResult<Self, String> {
        let id = whole(raw.id, "id")?;
        let priority = match raw.priority {
            None => 1,
            some => whole(some, "priority")?,
        };
        let raw_action = raw.action.ok_or("the rule has no `action`")?;
        let headers = |list: Option<Vec<RawHeader>>, what: &str| -> StdResult<Vec<Header>, String> {
            if list.as_ref().is_some_and(Vec::is_empty) {
                return Err(format!("`{what}` is an empty list, which both browsers refuse; leave it out instead"));
            }
            list.unwrap_or_default()
                .into_iter()
                .map(|h| {
                    let name = h.header.ok_or("a header change has no `header`")?;
                    let operation = match (h.operation.as_deref(), h.value) {
                        (Some("set"), Some(value)) => HeaderOperation::Set(value),
                        (Some("append"), Some(value)) => HeaderOperation::Append(value),
                        (Some("remove"), None) => HeaderOperation::Remove,
                        (Some("remove"), Some(_)) => return Err(format!("removing header `{name}` can't have a `value`")),
                        (Some(op @ ("set" | "append")), None) => return Err(format!("`{op}` for header `{name}` needs a `value`")),
                        (op, _) => return Err(format!("header `{name}` has operation {op:?}; use \"set\", \"append\" or \"remove\"")),
                    };
                    Ok(Header { name, operation })
                })
                .collect()
        };
        let kind = raw_action.kind.clone().unwrap_or_default();
        if kind != "redirect" && raw_action.redirect.is_some() {
            return Err(format!("a `{kind}` rule can't have `redirect`"));
        }
        if kind != "modifyHeaders" && (raw_action.request_headers.is_some() || raw_action.response_headers.is_some()) {
            return Err(format!("a `{kind}` rule can't change headers; use `modifyHeaders`"));
        }
        let action = match kind.as_str() {
            "block" => Action::Block,
            "allow" => Action::Allow,
            "allowAllRequests" => Action::AllowAllRequests,
            "upgradeScheme" => Action::UpgradeScheme,
            "modifyHeaders" => Action::ModifyHeaders(Headers {
                request: headers(raw_action.request_headers, "requestHeaders")?,
                response: headers(raw_action.response_headers, "responseHeaders")?,
            }),
            "redirect" => {
                let r = raw_action.redirect.ok_or("a redirect rule needs `redirect`")?;
                let set = [r.url.is_some(), r.extension_path.is_some(), r.transform.is_some(), r.regex_substitution.is_some()].iter().filter(|x| **x).count();
                if set != 1 {
                    return Err("`redirect` needs exactly one of `url`, `extensionPath`, `transform` or `regexSubstitution`; Firefox refuses more than one".to_string());
                }
                Action::Redirect(if let Some(url) = r.url {
                    Redirect::Url(url)
                } else if let Some(path) = r.extension_path {
                    Redirect::ExtensionPath(path)
                } else if let Some(substitution) = r.regex_substitution {
                    Redirect::RegexSubstitution(substitution)
                } else {
                    let t = r.transform.unwrap_or_default();
                    let query = match (t.query, t.query_transform) {
                        (Some(_), Some(_)) => return Err("the transform can't have both `query` and `queryTransform`".to_string()),
                        (Some(query), None) => Some(Query::Replace(query)),
                        (None, Some(qt)) => Some(Query::Edit {
                            remove: qt.remove_params.unwrap_or_default(),
                            add_or_replace: qt
                                .add_or_replace_params
                                .unwrap_or_default()
                                .into_iter()
                                .map(|p| QueryParam { key: p.key, value: p.value, replace_only: p.replace_only.unwrap_or(false) })
                                .collect(),
                        }),
                        (None, None) => None,
                    };
                    Redirect::Transform(Transform {
                        scheme: match t.scheme.as_deref() {
                            None => None,
                            Some("http") => Some(Scheme::Http),
                            Some("https") => Some(Scheme::Https),
                            Some(other) => return Err(format!("the transform scheme `{other}` isn't one both browsers take; use \"http\" or \"https\"")),
                        },
                        host: t.host,
                        port: match t.port.as_deref() {
                            None => None,
                            Some("") => Some(Port::Remove),
                            Some(port) => Some(Port::Set(port.parse().map_err(|_| format!("the transform port `{port}` isn't a number from 0 to 65535"))?)),
                        },
                        path: t.path,
                        query,
                        fragment: t.fragment,
                        username: t.username,
                        password: t.password,
                    })
                })
            }
            "" => return Err("the action has no `type`".to_string()),
            other => return Err(format!("`{other}` isn't an action type; use block, allow, allowAllRequests, upgradeScheme, redirect or modifyHeaders")),
        };
        let c = raw.condition.ok_or("the rule has no `condition`; use `{}` to match every request")?;
        for (list, what) in [
            (&c.request_domains, "requestDomains"),
            (&c.initiator_domains, "initiatorDomains"),
            (&c.resource_types, "resourceTypes"),
            (&c.request_methods, "requestMethods"),
        ] {
            empty_list(list, what)?;
        }
        let url = match (c.url_filter, c.regex_filter) {
            (Some(_), Some(_)) => return Err("a rule can have `urlFilter` or `regexFilter`, not both".to_string()),
            (Some(filter), None) => Some(UrlMatch::Filter(filter)),
            (None, Some(regex)) => Some(UrlMatch::Regex(regex)),
            (None, None) => None,
        };
        let condition = Condition {
            url,
            case_sensitive: c.is_url_filter_case_sensitive.unwrap_or(false),
            request_domains: c.request_domains,
            excluded_request_domains: c.excluded_request_domains.unwrap_or_default(),
            initiator_domains: c.initiator_domains,
            excluded_initiator_domains: c.excluded_initiator_domains.unwrap_or_default(),
            resource_types: c.resource_types.map(|list| parse_list(Some(list), "resourceTypes", ResourceType::parse, type_hint)).transpose()?,
            excluded_resource_types: parse_list(c.excluded_resource_types, "excludedResourceTypes", ResourceType::parse, type_hint)?,
            methods: c.request_methods.map(|list| parse_list(Some(list), "requestMethods", Method::parse, method_hint)).transpose()?,
            excluded_methods: parse_list(c.excluded_request_methods, "excludedRequestMethods", Method::parse, method_hint)?,
            domain_type: match c.domain_type.as_deref() {
                None => None,
                Some(text) => Some(DomainType::parse(text).ok_or_else(|| format!("`{text}` isn't a domain type; use \"firstParty\" or \"thirdParty\""))?),
            },
            tab_ids: c.tab_ids,
            excluded_tab_ids: c.excluded_tab_ids.unwrap_or_default(),
        };
        Ok(Rule { id, priority, action, condition })
    }
}

const RULE_KEYS: &[&str] = &["id", "priority", "action", "condition"];
const ACTION_KEYS: &[&str] = &["type", "redirect", "requestHeaders", "responseHeaders"];
const REDIRECT_KEYS: &[&str] = &["url", "extensionPath", "transform", "regexSubstitution"];
const TRANSFORM_KEYS: &[&str] = &["scheme", "host", "port", "path", "query", "queryTransform", "fragment", "username", "password"];
const QUERY_TRANSFORM_KEYS: &[&str] = &["removeParams", "addOrReplaceParams"];
const PARAM_KEYS: &[&str] = &["key", "value", "replaceOnly"];
const HEADER_KEYS: &[&str] = &["header", "operation", "value"];
const CONDITION_KEYS: &[&str] = &[
    "urlFilter",
    "regexFilter",
    "isUrlFilterCaseSensitive",
    "requestDomains",
    "excludedRequestDomains",
    "initiatorDomains",
    "excludedInitiatorDomains",
    "resourceTypes",
    "excludedResourceTypes",
    "requestMethods",
    "excludedRequestMethods",
    "domainType",
    "tabIds",
    "excludedTabIds",
];

fn known_keys(value: &serde_json::Value, keys: &[&str], at: &str) -> StdResult<(), String> {
    let Some(map) = value.as_object() else {
        return Err(format!("`{at}` must be an object"));
    };
    for key in map.keys() {
        if !keys.contains(&key.as_str()) {
            let why = match key.as_str() {
                "domains" | "excludedDomains" => format!("`{key}` is the old name Firefox doesn't know; use `{}`", if key == "domains" { "initiatorDomains" } else { "excludedInitiatorDomains" }),
                "responseHeaders" if at == "condition" => "matching on response headers only works in Chrome".to_string(),
                _ => format!("`{key}` isn't a key the browsers know here; the keys are {}", keys.join(", ")),
            };
            return Err(format!("{at}: {why}"));
        }
    }
    Ok(())
}

fn check_keys(rule: &serde_json::Value) -> StdResult<(), String> {
    known_keys(rule, RULE_KEYS, "the rule")?;
    if let Some(action) = rule.get("action") {
        known_keys(action, ACTION_KEYS, "action")?;
        if let Some(redirect) = action.get("redirect") {
            known_keys(redirect, REDIRECT_KEYS, "action.redirect")?;
            if let Some(transform) = redirect.get("transform") {
                known_keys(transform, TRANSFORM_KEYS, "action.redirect.transform")?;
                if let Some(qt) = transform.get("queryTransform") {
                    known_keys(qt, QUERY_TRANSFORM_KEYS, "action.redirect.transform.queryTransform")?;
                    for param in qt.get("addOrReplaceParams").and_then(|p| p.as_array()).into_iter().flatten() {
                        known_keys(param, PARAM_KEYS, "action.redirect.transform.queryTransform.addOrReplaceParams")?;
                    }
                }
            }
        }
        for list in ["requestHeaders", "responseHeaders"] {
            for header in action.get(list).and_then(|h| h.as_array()).into_iter().flatten() {
                known_keys(header, HEADER_KEYS, &format!("action.{list}"))?;
            }
        }
    }
    if let Some(condition) = rule.get("condition") {
        known_keys(condition, CONDITION_KEYS, "condition")?;
    }
    Ok(())
}

#[doc(hidden)]
#[derive(Debug, Default, PartialEq)]
pub struct StaticRuleSet {
    pub rules: usize,
    pub regex_rules: usize,
    pub extension_paths: Vec<String>,
    pub needs_host_access: bool,
}

#[doc(hidden)]
pub fn check_static_rule_set(text: &str) -> StdResult<StaticRuleSet, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| format!("isn't valid JSON: {err}"))?;
    let list = value.as_array().ok_or("must be a JSON list of rules, like `[{ \"id\": 1, ... }]`")?;
    let mut summary = StaticRuleSet::default();
    let mut ids = std::collections::BTreeSet::new();
    for (index, item) in list.iter().enumerate() {
        let label = match item.get("id").and_then(|id| id.as_f64()) {
            Some(id) => format!("rule {id} (number {} in the list)", index + 1),
            None => format!("rule number {} in the list", index + 1),
        };
        check_keys(item).map_err(|why| format!("{label}: {why}"))?;
        let raw: RawRule = serde_json::from_value(item.clone()).map_err(|err| format!("{label}: {err}"))?;
        let rule = Rule::try_from(raw).map_err(|why| format!("{label}: {why}"))?;
        rule.check(Scope::Static).map_err(|why| format!("{label}: {}", why.trim_start_matches(&format!("rule {}: ", rule.id))))?;
        if !ids.insert(rule.id) {
            return Err(format!("{label}: the id {} is used twice in this file", rule.id));
        }
        summary.rules += 1;
        summary.regex_rules += rule.is_regex() as usize;
        summary.needs_host_access |= rule.needs_host_access();
        if let Some(path) = rule.extension_path() {
            if !summary.extension_paths.iter().any(|p| p == path) {
                summary.extension_paths.push(path.to_string());
            }
        }
    }
    if summary.regex_rules > MAX_REGEX_RULES {
        return Err(format!("has {} regex rules; both browsers allow at most {MAX_REGEX_RULES} in one rule set", summary.regex_rules));
    }
    Ok(summary)
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    match message.as_str() {
        "oxichrome:no-permission" => OxichromeError::InvalidArgument(
            "the declarativeNetRequest API isn't available; add `DeclarativeNetRequest` to `permissions` in #[oxichrome::extension]".to_string(),
        ),
        "oxichrome:chrome-only" => OxichromeError::InvalidArgument("only Chrome has this; Firefox doesn't report which rules matched".to_string()),
        _ => OxichromeError::Js(JsError::new(JsValue::from_str(&message))),
    }
}

async fn call(method: &str, arg: JsValue) -> Result<JsValue> {
    let promise = js_bridge::dnr_call(method, &arg).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)
}

fn invalid(why: impl Into<String>) -> OxichromeError {
    OxichromeError::InvalidArgument(why.into())
}

async fn read_rules(method: &str) -> Result<Vec<Rule>> {
    let value = call(method, JsValue::UNDEFINED).await?;
    let raw: Vec<RawRule> = crate::json::from_js(&value)?;
    let mut rules = raw.into_iter().map(|raw| Rule::try_from(raw).map_err(OxichromeError::UnexpectedValue)).collect::<Result<Vec<_>>>()?;
    rules.sort_by_key(|rule| rule.id);
    Ok(rules)
}

pub async fn dynamic_rules() -> Result<Vec<Rule>> {
    read_rules("getDynamicRules").await
}

pub async fn session_rules() -> Result<Vec<Rule>> {
    read_rules("getSessionRules").await
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    declarative_net_request: Option<ManifestDnr>,
    #[serde(default)]
    web_accessible_resources: Vec<ManifestResources>,
}

#[derive(Deserialize)]
struct ManifestDnr {
    #[serde(default)]
    rule_resources: Vec<ManifestRuleSet>,
}

#[derive(Deserialize)]
struct ManifestRuleSet {
    id: String,
    path: String,
}

#[derive(Deserialize)]
struct ManifestResources {
    #[serde(default)]
    resources: Vec<String>,
}

fn manifest() -> Result<Manifest> {
    crate::json::from_js(&js_bridge::dnr_manifest().map_err(browser_error)?)
}

fn glob(pattern: &str, path: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == path,
        Some((head, tail)) => path.len() >= head.len() + tail.len() && path.starts_with(head) && glob_rest(tail, &path[head.len()..]),
    }
}

fn glob_rest(pattern: &str, path: &str) -> bool {
    match pattern.split_once('*') {
        None => path.ends_with(pattern),
        Some((head, tail)) => (0..=path.len()).any(|at| path[at..].starts_with(head) && glob_rest(tail, &path[at + head.len()..])),
    }
}

async fn check_update(scope: Scope, remove: &[u32], add: &[Rule]) -> Result<()> {
    for rule in add {
        rule.check(scope).map_err(invalid)?;
    }
    let mut seen = std::collections::BTreeSet::new();
    if let Some(twice) = add.iter().find(|rule| !seen.insert(rule.id)) {
        return Err(invalid(format!("rule {} is added twice", twice.id)));
    }
    if add.is_empty() && remove.is_empty() {
        return Ok(());
    }
    if add.iter().any(Rule::needs_host_access) {
        let granted = js_bridge::dnr_has_host_access().map_err(browser_error)?;
        if JsFuture::from(granted).await.map_err(browser_error)?.as_bool() == Some(false) {
            return Err(invalid(
                "redirect and modify_headers rules only work on sites the extension has host permission for, and it has none granted; list sites in `host_permissions` in #[oxichrome::extension] (Firefox also lets the user turn them off)",
            ));
        }
    }
    let paths: Vec<&str> = add.iter().filter_map(Rule::extension_path).collect();
    if !paths.is_empty() {
        let resources: Vec<String> = manifest()?.web_accessible_resources.into_iter().flat_map(|r| r.resources).collect();
        for path in paths {
            let inside = path.trim_start_matches('/');
            if !resources.iter().any(|pattern| glob(pattern.trim_start_matches('/'), inside)) {
                return Err(invalid(format!(
                    "Firefox only redirects to extension pages listed as web accessible; add `\"{inside}\"` to `web_accessible` in #[oxichrome::extension]"
                )));
            }
        }
    }
    let (current, other) = match scope {
        Scope::Session => (session_rules().await?, dynamic_rules().await?),
        _ => (dynamic_rules().await?, session_rules().await?),
    };
    if let Some(taken) = add.iter().find(|rule| current.iter().any(|c| c.id == rule.id) && !remove.contains(&rule.id)) {
        return Err(invalid(format!("rule {} is already there; pass its id in `remove` too to replace it", taken.id)));
    }
    let kept: Vec<&Rule> = current.iter().filter(|rule| !remove.contains(&rule.id)).collect();
    let total = kept.len() + add.len();
    let limit = if scope == Scope::Session { MAX_SESSION_RULES } else { MAX_DYNAMIC_RULES };
    if total > limit {
        return Err(OxichromeError::LimitExceeded(format!(
            "this would make {total} {} rules; Firefox allows at most {limit}",
            scope.name()
        )));
    }
    let regex = kept.iter().copied().chain(&other).filter(|rule| rule.is_regex()).count() + add.iter().filter(|rule| rule.is_regex()).count();
    if regex > MAX_REGEX_RULES {
        return Err(OxichromeError::LimitExceeded(format!(
            "this would make {regex} regex rules across dynamic and session rules; keep them at {MAX_REGEX_RULES} or fewer, which both browsers allow"
        )));
    }
    Ok(())
}

fn update_options(remove: &[u32], add: &[Rule]) -> Result<JsValue> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Update<'a> {
        remove_rule_ids: &'a [u32],
        add_rules: Vec<RawRule>,
    }
    crate::json::to_js(&Update { remove_rule_ids: remove, add_rules: add.iter().map(RawRule::from).collect() })
}

pub async fn update_dynamic_rules(remove: &[u32], add: &[Rule]) -> Result<()> {
    check_update(Scope::Dynamic, remove, add).await?;
    call("updateDynamicRules", update_options(remove, add)?).await.map(|_| ())
}

pub async fn update_session_rules(remove: &[u32], add: &[Rule]) -> Result<()> {
    check_update(Scope::Session, remove, add).await?;
    call("updateSessionRules", update_options(remove, add)?).await.map(|_| ())
}

pub async fn clear_dynamic_rules() -> Result<()> {
    let ids: Vec<u32> = dynamic_rules().await?.iter().map(|rule| rule.id).collect();
    update_dynamic_rules(&ids, &[]).await
}

pub async fn clear_session_rules() -> Result<()> {
    let ids: Vec<u32> = session_rules().await?.iter().map(|rule| rule.id).collect();
    update_session_rules(&ids, &[]).await
}

fn rule_set_ids() -> Result<Vec<(String, String)>> {
    Ok(manifest()?.declarative_net_request.map(|dnr| dnr.rule_resources.into_iter().map(|r| (r.id, r.path)).collect()).unwrap_or_default())
}

fn known_rule_set<'a>(sets: &'a [(String, String)], id: &str) -> Result<&'a str> {
    match sets.iter().find(|(known, _)| known == id) {
        Some((_, path)) => Ok(path),
        None if sets.is_empty() => Err(invalid(format!("there's no rule set `{id}`; add `rule_sets = {{ {id} = \"rules/{id}.json\" }}` to #[oxichrome::extension]"))),
        None => Err(invalid(format!(
            "there's no rule set `{id}`; the rule sets are {}",
            sets.iter().map(|(known, _)| format!("`{known}`")).collect::<Vec<_>>().join(", ")
        ))),
    }
}

pub async fn enabled_rule_sets() -> Result<Vec<String>> {
    let value = call("getEnabledRulesets", JsValue::UNDEFINED).await?;
    let mut ids: Vec<String> = crate::json::from_js(&value)?;
    ids.sort();
    Ok(ids)
}

pub async fn update_rule_sets(enable: &[&str], disable: &[&str]) -> Result<()> {
    let sets = rule_set_ids()?;
    for id in enable.iter().chain(disable) {
        known_rule_set(&sets, id)?;
    }
    if let Some(both) = enable.iter().find(|id| disable.contains(id)) {
        return Err(invalid(format!("rule set `{both}` is both enabled and disabled")));
    }
    let mut enabled = enabled_rule_sets().await?;
    enabled.retain(|id| !disable.contains(&id.as_str()));
    for id in enable {
        if !enabled.iter().any(|e| e == id) {
            enabled.push(id.to_string());
        }
    }
    if enabled.len() > MAX_ENABLED_RULE_SETS {
        return Err(OxichromeError::LimitExceeded(format!(
            "this would enable {} rule sets; Firefox allows at most {MAX_ENABLED_RULE_SETS} at once",
            enabled.len()
        )));
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Update<'a> {
        enable_ruleset_ids: &'a [&'a str],
        disable_ruleset_ids: &'a [&'a str],
    }
    call("updateEnabledRulesets", crate::json::to_js(&Update { enable_ruleset_ids: enable, disable_ruleset_ids: disable })?).await.map(|_| ())
}

async fn static_ids(path: &str) -> Result<Vec<u32>> {
    let promise = js_bridge::dnr_read_file(path).map_err(browser_error)?;
    let text = JsFuture::from(promise).await.map_err(browser_error)?.as_string().unwrap_or_default();
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let list: Vec<serde_json::Value> = serde_json::from_str(text).map_err(|err| OxichromeError::Serde(err.to_string()))?;
    Ok(list.iter().filter_map(|rule| rule.get("id").and_then(|id| id.as_u64())).map(|id| id as u32).collect())
}

pub async fn disabled_static_rules(rule_set: &str) -> Result<Vec<u32>> {
    let sets = rule_set_ids()?;
    known_rule_set(&sets, rule_set)?;
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Filter<'a> {
        ruleset_id: &'a str,
    }
    let value = call("getDisabledRuleIds", crate::json::to_js(&Filter { ruleset_id: rule_set })?).await?;
    let mut ids: Vec<u32> = crate::json::from_js(&value)?;
    ids.sort_unstable();
    Ok(ids)
}

async fn update_static(rule_set: &str, ids: &[u32], disable: bool) -> Result<()> {
    let sets = rule_set_ids()?;
    let path = known_rule_set(&sets, rule_set)?.to_string();
    if ids.is_empty() {
        return Ok(());
    }
    let known = static_ids(&path).await?;
    if let Some(missing) = ids.iter().find(|id| !known.contains(id)) {
        return Err(invalid(format!("rule set `{rule_set}` has no rule {missing}")));
    }
    if disable {
        let mut all = std::collections::BTreeSet::new();
        for (id, _) in &sets {
            all.extend(disabled_static_rules(id).await?.into_iter().map(|rule| (id.clone(), rule)));
        }
        all.extend(ids.iter().map(|rule| (rule_set.to_string(), *rule)));
        if all.len() > MAX_DISABLED_STATIC_RULES {
            return Err(OxichromeError::LimitExceeded(format!(
                "this would turn off {} static rules; both browsers allow at most {MAX_DISABLED_STATIC_RULES}",
                all.len()
            )));
        }
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Update<'a> {
        ruleset_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_rule_ids: Option<&'a [u32]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enable_rule_ids: Option<&'a [u32]>,
    }
    let update = Update { ruleset_id: rule_set, disable_rule_ids: disable.then_some(ids), enable_rule_ids: (!disable).then_some(ids) };
    call("updateStaticRules", crate::json::to_js(&update)?).await.map(|_| ())
}

pub async fn disable_static_rules(rule_set: &str, ids: &[u32]) -> Result<()> {
    update_static(rule_set, ids, true).await
}

pub async fn enable_static_rules(rule_set: &str, ids: &[u32]) -> Result<()> {
    update_static(rule_set, ids, false).await
}

pub async fn available_static_rule_count() -> Result<u32> {
    let value = call("getAvailableStaticRuleCount", JsValue::UNDEFINED).await?;
    value.as_f64().map(|n| n as u32).ok_or_else(|| OxichromeError::UnexpectedValue(format!("expected a number, got {value:?}")))
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedRule {
    pub rule_id: u32,
    pub rule_set: Option<String>,
    pub tab_id: i32,
    pub time: f64,
}

pub async fn matched_rules(tab_id: Option<i32>) -> Result<Vec<MatchedRule>> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Filter {
        #[serde(skip_serializing_if = "Option::is_none")]
        tab_id: Option<i32>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Info {
        rules_matched_info: Vec<Matched>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Matched {
        rule: MatchedId,
        tab_id: i32,
        time_stamp: f64,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MatchedId {
        rule_id: u32,
        ruleset_id: String,
    }
    let value = call("getMatchedRules", crate::json::to_js(&Filter { tab_id })?).await?;
    let info: Info = crate::json::from_js(&value)?;
    Ok(info
        .rules_matched_info
        .into_iter()
        .map(|m| MatchedRule {
            rule_id: m.rule.rule_id,
            rule_set: (!m.rule.ruleset_id.starts_with('_')).then_some(m.rule.ruleset_id),
            tab_id: m.tab_id,
            time: m.time_stamp,
        })
        .collect())
}

pub async fn show_match_count_on_badge(show: bool) -> Result<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Options {
        display_action_count_as_badge_text: bool,
    }
    call("setExtensionActionOptions", crate::json::to_js(&Options { display_action_count_as_badge_text: show })?).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(rule: Rule, scope: Scope) -> String {
        rule.check(scope).unwrap_err()
    }

    #[test]
    fn regexes_both_browsers_take() {
        for ok in ["a.*b", "^https?://(www\\.)?example\\.com/", "[a-z0-9_-]+", "a{1,1000}", "(a|b){2}(c){500}", "\\d\\w\\s\\.\\/\\-", "[\\]\\[]", "x\\x41", "(?:ab)+?", "[^/]*$"] {
            assert!(check_regex(ok).is_ok(), "{ok}: {:?}", check_regex(ok));
        }
        for (bad, why) in [
            ("", "empty"),
            ("(", "without a matching `)`"),
            (")", "without a matching `(`"),
            ("a(?=b)", "ahead or behind"),
            ("(?<!a)b", "ahead or behind"),
            ("(a)\\1", "refers back"),
            ("(?<n>a)", "names a group"),
            ("(?P<n>a)", "names a group"),
            ("(?i)a", "flags"),
            ("(a{1000}){1000}", "in all"),
            ("a{1001}", "at most 1000"),
            ("a{5,2}", "larger number first"),
            ("a**", "repeats a repeat"),
            ("*a", "nothing before it"),
            ("a{", "doesn't start a repeat"),
            ("[[:alpha:]]", "Firefox can't read"),
            ("[]", "empty `[]`"),
            ("[a", "without a matching `]`"),
            ("é", "outside ASCII"),
            ("\\A", "doesn't understand"),
            ("\\0", "\\x00"),
            ("a\\", "lone"),
            ("\\x4", "two hex digits"),
        ] {
            let got = check_regex(bad).unwrap_err();
            assert!(got.contains(why), "{bad}: {got}");
        }
        assert_eq!(check_regex("(a)(?:b)(c(d))").unwrap(), 3);
        assert!(check_regex("((a{10}){10}){10}").is_ok());
        assert!(check_regex("((a{10}){10}){10}b{1000}").is_ok());
        assert!(check_regex("(((a{10}){10}){10}){2}").unwrap_err().contains("2000 times in all"));
        assert!(check_regex("(a{2}|b{600}){2}").unwrap_err().contains("1200 times in all"));
        assert!(check_regex("((((a)*)*)*)*").is_ok());
        assert!(check_regex(&format!("{}a{}", "(".repeat(200), ")".repeat(200))).is_ok());
        assert!(check_regex(&format!("{}a{}", "(".repeat(5000), ")".repeat(5000))).unwrap_err().contains("200 deep"));
    }

    #[test]
    fn rules_follow_the_stricter_browser() {
        assert!(Rule::block(1).url_filter("||ads.example^").check(Scope::Dynamic).is_ok());
        assert!(Rule::block(1).check(Scope::Dynamic).is_ok());
        for (rule, why) in [
            (Rule::block(0), "id must be"),
            (Rule::block(MAX_ID + 1), "id must be"),
            (Rule::block(1).priority(0), "priority"),
            (Rule::block(1).priority(MAX_ID + 1), "priority"),
            (Rule::block(1).url_filter(""), "empty"),
            (Rule::block(1).url_filter("||*x"), "`||*`"),
            (Rule::block(1).url_filter("é"), "ASCII"),
            (Rule::block(1).request_domains(["Example.com"]), "capital"),
            (Rule::block(1).request_domains(["http://a.com"]), "is a URL"),
            (Rule::block(1).request_domains(["a.com:80"]), "port"),
            (Rule::block(1).request_domains(["*.a.com"]), "write `a.com`"),
            (Rule::block(1).initiator_domains(["münchen.de"]), "punycode"),
            (Rule::block(1).resource_types([ResourceType::Image]).excluded_resource_types([ResourceType::Image]), "both in"),
            (Rule::block(1).methods([Method::Get]).excluded_methods([Method::Get]), "both in"),
            (Rule::allow_all_requests(1), "needs resource_types"),
            (Rule::allow_all_requests(1).resource_types([ResourceType::Image]), "only list"),
            (Rule::block(1).tab_ids([1]), "only work in session"),
            (Rule::redirect(1, Redirect::url("/rel")), "full URL"),
            (Rule::redirect(1, Redirect::url("javascript:x")), "javascript"),
            (Rule::redirect(1, Redirect::url("https://a.com/x y")), "spaces"),
            (Rule::redirect(1, Redirect::extension_path("page.html")), "start with `/`"),
            (Rule::redirect(1, Redirect::regex_substitution("https://a/\\1")).url_filter("x"), "needs the rule to match"),
            (Rule::redirect(1, Redirect::regex_substitution("https://a/\\2")).regex("^http://x/(.*)"), "has 1 group"),
            (Rule::redirect(1, Redirect::regex_substitution("https://a/\\q")).regex("(.*)"), "isn't followed"),
            (Rule::redirect(1, Redirect::transform(Transform::new())), "doesn't change anything"),
            (Rule::redirect(1, Redirect::transform(Transform::new().query("q=1"))), "start with `?`"),
            (Rule::redirect(1, Redirect::transform(Transform::new().fragment("f"))), "start with `#`"),
            (Rule::redirect(1, Redirect::transform(Transform::new().path("p"))), "start with `/`"),
            (Rule::redirect(1, Redirect::transform(Transform::new().host("A.com"))), "capital"),
            (Rule::redirect(1, Redirect::transform(Transform::new().remove_param(""))), "can't be empty"),
            (Rule::modify_headers(1, Headers::new()), "at least one header"),
            (Rule::modify_headers(1, Headers::new().set_request("bad header", "v")), "header names can't have"),
            (Rule::modify_headers(1, Headers::new().set_request("", "v")), "can't be empty"),
            (Rule::modify_headers(1, Headers::new().set_request("x-a", "a\nb")), "line break"),
            (Rule::modify_headers(1, Headers::new().append_request("x-a", "v")), "only appends"),
        ] {
            let got = err(rule.clone(), Scope::Dynamic);
            assert!(got.contains(why), "{rule:?}: {got}");
        }
        assert!(Rule::block(1).tab_ids([1, -1]).check(Scope::Session).is_ok());
        for rule in [
            Rule::block(1).request_domains(Vec::<String>::new()),
            Rule::block(1).initiator_domains(Vec::<&str>::new()),
            Rule::block(1).resource_types([]),
            Rule::block(1).methods([]),
            Rule::allow_all_requests(1).resource_types([]),
        ] {
            assert!(err(rule.clone(), Scope::Dynamic).contains("empty list"), "{rule:?}");
        }
        assert!(err(Rule::block(1).tab_ids([]), Scope::Session).contains("empty list"));
        assert!(Rule::block(1).excluded_resource_types([]).excluded_methods([]).check(Scope::Dynamic).is_ok());
        assert!(Rule::modify_headers(1, Headers::new().append_request("Accept", "v").append_response("x-a", "v").remove_request("user-agent")).check(Scope::Dynamic).is_ok());
        assert!(Rule::block(1).request_domains(["127.0.0.1", "[::1]", "xn--mnchen-3ya.de"]).check(Scope::Dynamic).is_ok());
        assert!(Rule::redirect(1, Redirect::regex_substitution("https://a/\\1\\0\\\\")).regex("^http://x/(.*)").check(Scope::Dynamic).is_ok());
        assert!(Rule::redirect(1, Redirect::transform(Transform::new().scheme(Scheme::Https).remove_port().query("").set_param("a", "1").remove_param("b"))).check(Scope::Dynamic).is_ok());
        assert!(Rule::allow_all_requests(1).resource_types([ResourceType::MainFrame, ResourceType::SubFrame]).check(Scope::Dynamic).is_ok());
    }

    fn roundtrip(rule: &Rule) -> Rule {
        let text = serde_json::to_string(&RawRule::from(rule)).unwrap();
        let raw: RawRule = serde_json::from_str(&text).unwrap();
        Rule::try_from(raw).unwrap()
    }

    #[test]
    fn rules_survive_the_trip_to_the_browser_and_back() {
        let rules = [
            Rule::block(1).url_filter("x").priority(4).case_sensitive().resource_types([ResourceType::Image]).excluded_methods([Method::Post]).third_party(),
            Rule::redirect(2, Redirect::transform(Transform::new().scheme(Scheme::Http).host("a.com").port(81).path("/p").fragment("").set_param("k", "v").replace_param("r", "1").remove_param("z").username("u").password("p"))),
            Rule::redirect(3, Redirect::transform(Transform::new().remove_port().query("?a"))),
            Rule::redirect(4, Redirect::regex_substitution("https://a/\\1")).regex("(.*)"),
            Rule::redirect(5, Redirect::extension_path("/x.html")),
            Rule::redirect(6, Redirect::url("https://b.com/")),
            Rule::modify_headers(7, Headers::new().set_request("a", "1").append_request("accept", "2").remove_request("c").set_response("d", "4").remove_response("e")),
            Rule::allow_all_requests(8).resource_types([ResourceType::MainFrame]).initiator_domains(["a.com"]).excluded_initiator_domains(["b.a.com"]).request_domains(["c.com"]).excluded_request_domains(["d.c.com"]),
            Rule::upgrade_scheme(9).first_party().tab_ids([1]).excluded_tab_ids([2]),
            Rule::allow(10),
        ];
        for rule in &rules {
            assert_eq!(&roundtrip(rule), rule);
        }
    }

    #[test]
    fn firefox_nulls_read_as_missing() {
        let raw: RawRule = serde_json::from_str(r#"{"id":5,"priority":1,"condition":{"urlFilter":"x","regexFilter":null,"resourceTypes":null,"tabIds":null},"action":{"type":"block","redirect":null,"requestHeaders":null}}"#).unwrap();
        assert_eq!(Rule::try_from(raw).unwrap(), Rule::block(5).url_filter("x"));
    }

    #[test]
    fn static_files_are_checked_like_runtime_rules() {
        let ok = r#"[{"id":1,"priority":1,"action":{"type":"block"},"condition":{"urlFilter":"||a.com^","resourceTypes":["script"]}},
            {"id":2,"action":{"type":"redirect","redirect":{"extensionPath":"/blocked.html"}},"condition":{"urlFilter":"b","resourceTypes":["main_frame"]}},
            {"id":3,"action":{"type":"block"},"condition":{"regexFilter":"^x"}}]"#;
        let summary = check_static_rule_set(ok).unwrap();
        assert_eq!(summary, StaticRuleSet { rules: 3, regex_rules: 1, extension_paths: vec!["/blocked.html".into()], needs_host_access: true });
        for (text, why) in [
            ("{", "valid JSON"),
            ("{}", "JSON list"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{},"foo":1}]"#, "`foo` isn't a key"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"domains":["a.com"]}}]"#, "old name"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"responseHeaders":[]}}]"#, "only works in Chrome"),
            (r#"[{"id":1,"action":{"type":"block"}}]"#, "no `condition`"),
            (r#"[{"id":1,"condition":{}}]"#, "no `action`"),
            (r#"[{"action":{"type":"block"},"condition":{}}]"#, "no id"),
            (r#"[{"id":"1","action":{"type":"block"},"condition":{}}]"#, "invalid type"),
            (r#"[{"id":1.5,"action":{"type":"block"},"condition":{}}]"#, "whole number"),
            (r#"[{"id":1,"action":{"type":"nope"},"condition":{}}]"#, "isn't an action type"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"resourceTypes":[]}}]"#, "empty list"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"resourceTypes":["beacon"]}}]"#, "only Firefox"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"resourceTypes":["webtransport"]}}]"#, "only Chrome"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"requestMethods":["GET"]}}]"#, "lower case"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"tabIds":[1]}}]"#, "only work in session"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"urlFilter":"a","regexFilter":"a"}}]"#, "not both"),
            (r#"[{"id":1,"action":{"type":"redirect","redirect":{"url":"https://a/","extensionPath":"/x"}},"condition":{}}]"#, "exactly one"),
            (r#"[{"id":1,"action":{"type":"redirect","redirect":{"transform":{"scheme":"ftp"}}},"condition":{}}]"#, "http"),
            (r#"[{"id":1,"action":{"type":"modifyHeaders","requestHeaders":[{"header":"a","operation":"set"}]},"condition":{}}]"#, "needs a `value`"),
            (r#"[{"id":1,"action":{"type":"modifyHeaders","requestHeaders":[{"header":"a","operation":"remove","value":"x"}]},"condition":{}}]"#, "can't have a `value`"),
            (r#"[{"id":1,"action":{"type":"block","requestHeaders":[{"header":"a","operation":"remove"}]},"condition":{}}]"#, "can't change headers"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{}},{"id":1,"action":{"type":"block"},"condition":{}}]"#, "used twice"),
            (r#"[{"id":1,"action":{"type":"block"},"condition":{"regexFilter":"(?=a)"}}]"#, "ahead or behind"),
        ] {
            let got = check_static_rule_set(text).unwrap_err();
            assert!(got.contains(why), "{text}: {got}");
        }
        let many: Vec<String> = (1..=1001).map(|id| format!(r#"{{"id":{id},"action":{{"type":"block"}},"condition":{{"regexFilter":"x{id}"}}}}"#)).collect();
        assert!(check_static_rule_set(&format!("[{}]", many.join(","))).unwrap_err().contains("at most 1000"));
    }

    #[test]
    fn globs_match_like_the_manifest() {
        assert!(glob("blocked.html", "blocked.html"));
        assert!(glob("pages/*", "pages/a/b.html"));
        assert!(glob("*.html", "x.html"));
        assert!(glob("a*b*c", "aXbYc"));
        assert!(!glob("pages/*", "other/a.html"));
        assert!(!glob("a*b", "ab_"));
    }
}
