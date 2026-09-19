use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitBool, LitStr, Token};

/// See <https://developer.chrome.com/docs/extensions/develop/concepts/match-patterns> from <https://developer.chrome.com/docs/extensions/develop/concepts/declare-permissions#host-permissions>
/// and also <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Match_patterns#invalid_or_unmatched_patterns> from <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json/host_permissions>
fn validate_host_permission_pattern(pattern: &LitStr) -> syn::Result<()> {
    let val = pattern.value();
    if val == "<all_urls>" {
        return Ok(());
    }

    let scheme_end = if val.starts_with("https://") {
        8
    } else if val.starts_with("http://") {
        7
    } else if val.starts_with("*://") {
        4
    } else if val.starts_with("file:///") {
        8
    } else if val.starts_with("ftp://") {
        6
    } else {
        return Err(syn::Error::new(
            pattern.span(),
            format!(
                "invalid host_permission pattern `{val}`: must start with a scheme \
                 (e.g. `https://`, `*://`, `http://`, `file:///`) or be `<all_urls>` \
                See https://developer.chrome.com/docs/extensions/develop/concepts/match-patterns \
                and/or https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Match_patterns#invalid_or_unmatched_patterns"
            ),
        ));
    };

    // Check for fragment identifiers (not allowed)
    if val.contains('#') {
        return Err(syn::Error::new(
            pattern.span(),
            format!(
                "invalid host_permission pattern `{val}`: fragment identifiers (#) are not allowed \
                 in match patterns"
            ),
        ));
    }

    let rest = &val[scheme_end..];

    if rest.starts_with('/') {
        return Err(syn::Error::new(
            pattern.span(),
            format!("invalid host_permission pattern `{val}`: host cannot be empty"),
        ));
    }

    let path_start = rest.find('/');
    if path_start.is_none() {
        return Err(syn::Error::new(
            pattern.span(),
            format!(
                "invalid host_permission pattern `{val}`: must include a path starting with '/' \
                 (e.g. `https://example.org/`, `*://*.example.org/*`)"
            ),
        ));
    }

    let host_and_port = &rest[..path_start.unwrap()];
    let (host, port) = if let Some(colon_pos) = host_and_port.rfind(':') {
        (
            &host_and_port[..colon_pos],
            Some(&host_and_port[colon_pos + 1..]),
        )
    } else {
        (host_and_port, None)
    };

    if host.contains('*') {
        // Wildcard must be at the start...
        if !host.starts_with('*') {
            return Err(syn::Error::new(
                pattern.span(),
                format!(
                    "invalid host_permission pattern `{val}`: wildcard (*) in host must be at the start"
                ),
            ));
        }
        // ...and must be alone or followed by period mark
        if host != "*" && !host.starts_with("*.") {
            return Err(syn::Error::new(
                pattern.span(),
                format!(
                    "invalid host_permission pattern `{val}`: wildcard (*) must be alone or followed by a dot (e.g. `*` or `*.example.org`)"
                ),
            ));
        }
    }

    // Validate port if present (Chrome allows * for port)
    if let Some(port_str) = port {
        if port_str != "*" && port_str.parse::<u16>().is_err() {
            return Err(syn::Error::new(
                pattern.span(),
                format!("invalid host_permission pattern `{val}`: invalid port `{port_str}`"),
            ));
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
    pub name: LitStr,
    pub version: LitStr,
    pub description: Option<LitStr>,
    pub permissions: Vec<LitStr>,
    pub host_permissions: Vec<LitStr>,
}

impl Parse for ExtensionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<LitStr> = None;
        let mut version: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut permissions: Vec<LitStr> = Vec::new();
        let mut host_permissions: Vec<LitStr> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "name" => {
                    name = Some(input.parse()?);
                }
                "version" => {
                    version = Some(input.parse()?);
                }
                "description" => {
                    description = Some(input.parse()?);
                }
                "gecko_id" => {
                    // validated but not stored — gecko_id is build-tool metadata
                    // (manifest.json generation), not needed for code generation
                    let gecko_id: LitStr = input.parse()?;
                    validate_gecko_id(&gecko_id)?;
                }
                "permissions" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        permissions.push(content.parse()?);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "host_permissions" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let pattern: LitStr = content.parse()?;
                        validate_host_permission_pattern(&pattern)?;
                        host_permissions.push(pattern);
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

        Ok(ExtensionArgs {
            name,
            version,
            description,
            permissions,
            host_permissions,
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

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "matches" => {
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

        if matches.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required argument `matches`",
            ));
        }

        for pattern in &matches {
            let val = pattern.value();
            if val != "<all_urls>"
                && !val.starts_with("http://")
                && !val.starts_with("https://")
                && !val.starts_with("*://")
                && !val.starts_with("file://")
                && !val.starts_with("ftp://")
            {
                return Err(syn::Error::new(
                    pattern.span(),
                    format!(
                        "invalid match pattern `{val}`: must start with a scheme \
                         (e.g. `https://`, `*://`, `http://`) or be `<all_urls>`"
                    ),
                ));
            }
        }

        Ok(ContentScriptArgs {
            matches,
            run_at,
            all_frames,
            css,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extension_args() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test Extension",
            version = "1.0.0",
            permissions = ["storage", "tabs"]
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
            permissions = ["storage"],
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
            "http://*:*/*",
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
            ("file://somehost/path", "invalid"),
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
}
