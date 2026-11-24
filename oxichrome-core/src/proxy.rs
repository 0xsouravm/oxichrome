//! Sending the browser's traffic through a proxy, the same way in Chrome and Firefox.
//!
//! Needs `Proxy` in `permissions`. In Firefox the user must also let the extension run in
//! private windows (about:addons, the extension, "Run in Private Windows"); until then
//! [`set`] fails and [`allowed`] is `false`. Addresses on this computer (`localhost`,
//! `127.0.0.1`, `[::1]`) never go through the proxy, in either browser.
//!
//! ```ignore
//! use proxy::{Config, Server, Servers};
//!
//! proxy::set(Config::Servers(Servers::all(Server::http("proxy.example.com", 8080)).bypass("*.internal"))).await?;
//! proxy::set(Config::Pac(proxy::Pac::Script(r#"function FindProxyForURL(url, host) { return "DIRECT"; }"#.into()))).await?;
//! proxy::clear().await?; // back to the user's own setting
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::error::{JsError, OxichromeError, Result};
use crate::js_bridge;
pub use crate::privacy::{Control, State};
pub use crate::runtime::Listener;

/// What kind of proxy a [`Server`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Http,
    Socks4,
    /// SOCKS 5, which also looks up addresses through the proxy.
    Socks5,
}

/// One proxy server.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Server {
    pub kind: Kind,
    pub host: String,
    pub port: u16,
}

impl Server {
    pub fn http(host: impl Into<String>, port: u16) -> Self {
        Server { kind: Kind::Http, host: host.into(), port }
    }

    pub fn socks4(host: impl Into<String>, port: u16) -> Self {
        Server { kind: Kind::Socks4, host: host.into(), port }
    }

    pub fn socks5(host: impl Into<String>, port: u16) -> Self {
        Server { kind: Kind::Socks5, host: host.into(), port }
    }

    fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Which server each request uses, and which hosts skip the proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Servers {
    pub route: Route,
    /// Hosts that connect directly, like `example.com` or `*.internal`.
    pub bypass: Vec<String>,
}

/// Where requests go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Every request through one server.
    All(Server),
    /// `http:` and `https:` requests through their own HTTP proxies; a scheme without one
    /// connects directly.
    ByScheme { http: Option<Server>, https: Option<Server> },
}

impl Servers {
    /// Every request through `server`.
    pub fn all(server: Server) -> Self {
        Servers { route: Route::All(server), bypass: Vec::new() }
    }

    /// `http:` and `https:` through their own HTTP proxies.
    pub fn by_scheme(http: Option<Server>, https: Option<Server>) -> Self {
        Servers { route: Route::ByScheme { http, https }, bypass: Vec::new() }
    }

    /// Connect to `host` directly. Call it again for more hosts.
    pub fn bypass(mut self, host: impl Into<String>) -> Self {
        self.bypass.push(host.into());
        self
    }
}

/// A proxy auto-config file, which picks a proxy for each URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pac {
    /// The file at this URL.
    Url(String),
    /// The script itself, with its `FindProxyForURL(url, host)` function.
    Script(String),
}

/// How the browser connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Config {
    /// Straight to every site, no proxy.
    Direct,
    /// Whatever the operating system is set to.
    System,
    /// Look for a proxy on the network (WPAD).
    AutoDetect,
    Pac(Pac),
    Servers(Servers),
}

const PAC_PREFIX: &str = "data:application/x-ns-proxy-autoconfig;charset=utf-8,";

fn check(config: &Config) -> Result<()> {
    let bad = |message: String| Err(OxichromeError::InvalidArgument(message));
    match config {
        Config::Servers(servers) => {
            let servers_used: Vec<&Server> = match &servers.route {
                Route::All(server) => vec![server],
                Route::ByScheme { http, https } => {
                    if http.is_none() && https.is_none() {
                        return bad("`Servers::by_scheme` got no server for either scheme; use `Config::Direct` to connect directly".into());
                    }
                    let both: Vec<&Server> = http.iter().chain(https).collect();
                    if both.iter().any(|server| server.kind != Kind::Http) {
                        return bad("`Servers::by_scheme` takes HTTP proxies only; for SOCKS, send everything through it with `Servers::all`".into());
                    }
                    both
                }
            };
            for server in servers_used {
                if server.host.trim().is_empty() || server.host.contains(['/', ' ', ',']) {
                    return bad(format!("`{}` isn't a proxy host; give the host alone, like `proxy.example.com`", server.host));
                }
                if server.port == 0 {
                    return bad(format!("the proxy `{}` needs a port", server.host));
                }
            }
            if let Some(host) = servers.bypass.iter().find(|host| host.trim().is_empty() || host.contains([',', ' '])) {
                return bad(format!("`{host}` isn't a host to bypass; give one host per `bypass`, like `*.example.com`"));
            }
            Ok(())
        }
        Config::Pac(Pac::Url(url)) if url.trim().is_empty() => bad("the PAC file URL is empty".into()),
        Config::Pac(Pac::Script(script)) if !script.contains("FindProxyForURL") => {
            bad("the PAC script has no `FindProxyForURL(url, host)` function".into())
        }
        _ => Ok(()),
    }
}

fn chrome_server(server: &Server) -> Value {
    let scheme = match server.kind {
        Kind::Http => "http",
        Kind::Socks4 => "socks4",
        Kind::Socks5 => "socks5",
    };
    json!({ "scheme": scheme, "host": server.host, "port": server.port })
}

fn to_chrome(config: &Config) -> Value {
    match config {
        Config::Direct => json!({ "mode": "direct" }),
        Config::System => json!({ "mode": "system" }),
        Config::AutoDetect => json!({ "mode": "auto_detect" }),
        Config::Pac(Pac::Url(url)) => json!({ "mode": "pac_script", "pacScript": { "url": url } }),
        Config::Pac(Pac::Script(script)) => json!({ "mode": "pac_script", "pacScript": { "data": script } }),
        Config::Servers(servers) => {
            let mut rules = serde_json::Map::new();
            match &servers.route {
                Route::All(server) => {
                    rules.insert("singleProxy".into(), chrome_server(server));
                }
                Route::ByScheme { http, https } => {
                    if let Some(server) = http {
                        rules.insert("proxyForHttp".into(), chrome_server(server));
                    }
                    if let Some(server) = https {
                        rules.insert("proxyForHttps".into(), chrome_server(server));
                    }
                }
            }
            if !servers.bypass.is_empty() {
                rules.insert("bypassList".into(), json!(servers.bypass));
            }
            json!({ "mode": "fixed_servers", "rules": rules })
        }
    }
}

fn to_firefox(config: &Config) -> Value {
    match config {
        Config::Direct => json!({ "proxyType": "none" }),
        Config::System => json!({ "proxyType": "system" }),
        Config::AutoDetect => json!({ "proxyType": "autoDetect" }),
        Config::Pac(Pac::Url(url)) => json!({ "proxyType": "autoConfig", "autoConfigUrl": url }),
        Config::Pac(Pac::Script(script)) => json!({
            "proxyType": "autoConfig",
            "autoConfigUrl": format!("{PAC_PREFIX}{}", js_sys::encode_uri_component(script)),
        }),
        Config::Servers(servers) => {
            let mut value = json!({
                "proxyType": "manual",
                "http": "",
                "ssl": "",
                "socks": "",
                "httpProxyAll": false,
                "passthrough": servers.bypass.join(", "),
            });
            match &servers.route {
                Route::All(server) if server.kind == Kind::Http => {
                    value["http"] = json!(server.address());
                    value["httpProxyAll"] = json!(true);
                }
                Route::All(server) => {
                    value["socks"] = json!(server.address());
                    value["socksVersion"] = json!(if server.kind == Kind::Socks4 { 4 } else { 5 });
                    value["proxyDNS"] = json!(server.kind == Kind::Socks5);
                }
                Route::ByScheme { http, https } => {
                    value["http"] = json!(http.as_ref().map(Server::address).unwrap_or_default());
                    value["ssl"] = json!(https.as_ref().map(Server::address).unwrap_or_default());
                }
            }
            value
        }
    }
}

fn parse_address(address: &str, kind: Kind) -> Option<Server> {
    let address = address.split_once("://").map_or(address, |(_, rest)| rest).trim_end_matches('/');
    let (host, port) = address.rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some(Server { kind, host: host.to_string(), port: port.parse().ok()? })
}

fn split_bypass(text: &str) -> Vec<String> {
    text.split([',', ' ', ';']).map(str::trim).filter(|host| !host.is_empty()).map(String::from).collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChromeServer {
    #[serde(default)]
    scheme: Option<String>,
    host: String,
    #[serde(default)]
    port: Option<u16>,
}

impl ChromeServer {
    fn read(self) -> Server {
        let (kind, default_port) = match self.scheme.as_deref() {
            Some("socks4") => (Kind::Socks4, 1080),
            Some("socks5") => (Kind::Socks5, 1080),
            _ => (Kind::Http, 80),
        };
        Server { kind, host: self.host, port: self.port.unwrap_or(default_port) }
    }
}

fn from_chrome(value: &Value) -> Option<Config> {
    Some(match value.get("mode")?.as_str()? {
        "direct" => Config::Direct,
        "system" => Config::System,
        "auto_detect" => Config::AutoDetect,
        "pac_script" => {
            let pac = value.get("pacScript")?;
            match (pac.get("data").and_then(Value::as_str), pac.get("url").and_then(Value::as_str)) {
                (Some(data), _) if !data.is_empty() => Config::Pac(Pac::Script(data.to_string())),
                (_, Some(url)) => Config::Pac(Pac::Url(url.to_string())),
                _ => return None,
            }
        }
        "fixed_servers" => {
            let rules = value.get("rules")?;
            let server = |key: &str| rules.get(key).and_then(|raw| serde_json::from_value::<ChromeServer>(raw.clone()).ok()).map(ChromeServer::read);
            let route = match server("singleProxy") {
                Some(single) => Route::All(single),
                None => Route::ByScheme { http: server("proxyForHttp"), https: server("proxyForHttps") },
            };
            let bypass = rules
                .get("bypassList")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_str).map(String::from).collect())
                .unwrap_or_default();
            Config::Servers(Servers { route, bypass })
        }
        _ => return None,
    })
}

fn from_firefox(value: &Value) -> Option<Config> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default();
    Some(match text("proxyType") {
        "none" => Config::Direct,
        "system" => Config::System,
        "autoDetect" => Config::AutoDetect,
        "autoConfig" => {
            let url = text("autoConfigUrl");
            match url.strip_prefix(PAC_PREFIX) {
                Some(encoded) => Config::Pac(Pac::Script(js_sys::decode_uri_component(encoded).ok()?.into())),
                None => Config::Pac(Pac::Url(url.to_string())),
            }
        }
        "manual" => {
            let bypass = split_bypass(text("passthrough"));
            let all = value.get("httpProxyAll").and_then(Value::as_bool).unwrap_or(false);
            let route = if all && !text("http").is_empty() {
                Route::All(parse_address(text("http"), Kind::Http)?)
            } else if text("http").is_empty() && text("ssl").is_empty() && !text("socks").is_empty() {
                let kind = if value.get("socksVersion").and_then(Value::as_u64) == Some(4) { Kind::Socks4 } else { Kind::Socks5 };
                Route::All(parse_address(text("socks"), kind)?)
            } else {
                Route::ByScheme { http: parse_address(text("http"), Kind::Http), https: parse_address(text("ssl"), Kind::Http) }
            };
            Config::Servers(Servers { route, bypass })
        }
        _ => return None,
    })
}

fn no_permission() -> OxichromeError {
    OxichromeError::InvalidArgument("the proxy API isn't available; add `Proxy` to `permissions` in #[oxichrome::extension]".into())
}

fn browser_error(error: JsValue) -> OxichromeError {
    let message = JsError::new(error).to_string();
    if message == "oxichrome:no-permission" {
        return no_permission();
    }
    if message.contains("private browsing permission") {
        return OxichromeError::InvalidArgument(
            "Firefox lets an extension set the proxy only once the user allows it in private windows (about:addons, the extension, \"Run in Private Windows\"); check `proxy::allowed()` first".into(),
        );
    }
    OxichromeError::Js(JsError::new(JsValue::from_str(&message)))
}

/// Whether this extension may change the proxy: always in Chrome, and in Firefox once the
/// user lets it run in private windows.
pub async fn allowed() -> Result<bool> {
    let value = JsFuture::from(js_bridge::proxy_allowed()).await.map_err(browser_error)?;
    Ok(value.as_bool().unwrap_or(false))
}

/// How the browser connects now, and who decides it. `None` as the value means a setup
/// oxichrome doesn't read, made outside the extension.
pub async fn get() -> Result<State<Option<Config>>> {
    let promise = js_bridge::proxy_call("get", &JsValue::UNDEFINED).map_err(browser_error)?;
    let raw = JsFuture::from(promise).await.map_err(browser_error)?;
    let raw: Value = crate::json::from_js(&raw)?;
    let value = raw.get("value").cloned().unwrap_or(Value::Null);
    let config = if js_bridge::is_firefox() { from_firefox(&value) } else { from_chrome(&value) };
    let control = crate::privacy::control_of(raw.get("levelOfControl").and_then(Value::as_str).unwrap_or_default());
    Ok(State { value: config, control })
}

/// Makes the browser connect this way, for as long as the extension is installed or until
/// [`clear`]. Fails when a policy or another extension decides the proxy.
pub async fn set(config: Config) -> Result<()> {
    check(&config)?;
    let value = if js_bridge::is_firefox() { to_firefox(&config) } else { to_chrome(&config) };
    let value = crate::json::to_js(&value)?;
    let promise = js_bridge::proxy_call("set", &value).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)?;
    let now = get().await?;
    if now.control != Control::ControlledByThisExtension {
        return Err(OxichromeError::InvalidArgument(match now.control {
            Control::ControlledByOtherExtensions => "the proxy didn't change: another extension decides it".into(),
            _ => "the proxy didn't change: a policy or the browser decides it".into(),
        }));
    }
    Ok(())
}

/// Gives the proxy setting back to the user (or the next extension that set it).
pub async fn clear() -> Result<()> {
    let promise = js_bridge::proxy_call("clear", &JsValue::UNDEFINED).map_err(browser_error)?;
    JsFuture::from(promise).await.map_err(browser_error)?;
    Ok(())
}

/// A proxy that failed, passed to [`on_error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyError {
    pub message: String,
    /// Whether the browser gave up on the connection, rather than falling back.
    pub fatal: bool,
}

/// Runs `handler` each time a proxy can't be reached or a PAC script fails. Chrome only:
/// Firefox reports these to `proxy.onRequest` listeners alone, so there `handler` never runs.
pub fn on_error(handler: impl Fn(ProxyError) + 'static) -> Result<Listener> {
    let callback = Closure::<dyn Fn(JsValue)>::new(move |raw: JsValue| {
        let get = |key: &str| js_sys::Reflect::get(&raw, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED);
        let message = [get("error"), get("message"), get("details")]
            .iter()
            .filter_map(JsValue::as_string)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(": ");
        handler(ProxyError { message, fatal: get("fatal").as_bool().unwrap_or(false) });
    })
    .into_js_value();
    Listener::from_remover(js_bridge::proxy_on_error(&callback)).ok_or_else(no_permission)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(config: Config) {
        assert_eq!(from_chrome(&to_chrome(&config)).as_ref(), Some(&config), "chrome: {:?}", to_chrome(&config));
    }

    #[test]
    fn chrome_reads_back_what_it_was_given() {
        round_trip(Config::Direct);
        round_trip(Config::System);
        round_trip(Config::AutoDetect);
        round_trip(Config::Pac(Pac::Url("https://example.com/proxy.pac".into())));
        round_trip(Config::Pac(Pac::Script("function FindProxyForURL(u, h) { return 'DIRECT'; }".into())));
        round_trip(Config::Servers(Servers::all(Server::http("p.example", 8080)).bypass("*.internal").bypass("<local>")));
        round_trip(Config::Servers(Servers::all(Server::socks5("p.example", 1080))));
        round_trip(Config::Servers(Servers::by_scheme(Some(Server::http("a", 1)), None)));
    }

    #[test]
    fn firefox_values() {
        let all = to_firefox(&Config::Servers(Servers::all(Server::http("p.example", 8080)).bypass("a.com").bypass("*.b.com")));
        assert_eq!((all["http"].as_str(), all["httpProxyAll"].as_bool(), all["passthrough"].as_str()), (Some("p.example:8080"), Some(true), Some("a.com, *.b.com")));
        let socks = to_firefox(&Config::Servers(Servers::all(Server::socks4("s", 1080))));
        assert_eq!((socks["socks"].as_str(), socks["socksVersion"].as_u64(), socks["proxyDNS"].as_bool()), (Some("s:1080"), Some(4), Some(false)));
        let firefox_read = |value: Value| from_firefox(&value);
        let stored = json!({"proxyType":"manual","autoConfigUrl":"","autoLogin":false,"proxyDNS":true,"httpProxyAll":true,"socksVersion":5,"passthrough":"a.com, *.b.com","http":"p.example:8080","ssl":"","socks":""});
        assert_eq!(firefox_read(stored), Some(Config::Servers(Servers::all(Server::http("p.example", 8080)).bypass("a.com").bypass("*.b.com"))));
        let by_scheme = json!({"proxyType":"manual","httpProxyAll":false,"http":"h:1","ssl":"s:2","socks":"","passthrough":""});
        assert_eq!(firefox_read(by_scheme), Some(Config::Servers(Servers::by_scheme(Some(Server::http("h", 1)), Some(Server::http("s", 2))))));
        let socks = json!({"proxyType":"manual","httpProxyAll":false,"http":"","ssl":"","socks":"s:1080","socksVersion":5,"passthrough":""});
        assert_eq!(firefox_read(socks), Some(Config::Servers(Servers::all(Server::socks5("s", 1080)))));
        assert_eq!(firefox_read(json!({"proxyType":"none"})), Some(Config::Direct));
        assert_eq!(firefox_read(json!({"proxyType":"autoConfig","autoConfigUrl":"https://x/p.pac"})), Some(Config::Pac(Pac::Url("https://x/p.pac".into()))));
    }

    #[test]
    fn bad_configs_are_refused() {
        assert!(check(&Config::Servers(Servers::by_scheme(None, None))).is_err());
        assert!(check(&Config::Servers(Servers::by_scheme(Some(Server::socks5("s", 1)), None))).is_err());
        assert!(check(&Config::Servers(Servers::all(Server::http("http://p", 1)))).is_err());
        assert!(check(&Config::Servers(Servers::all(Server::http("p", 0)))).is_err());
        assert!(check(&Config::Servers(Servers::all(Server::http("p", 1)).bypass("a.com, b.com"))).is_err());
        assert!(check(&Config::Pac(Pac::Script("return 1".into()))).is_err());
        assert!(check(&Config::Servers(Servers::all(Server::http("p", 1)).bypass("*.a.com"))).is_ok());
    }
}
