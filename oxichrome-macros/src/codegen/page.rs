use std::path::Path;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::ParseStream;
use syn::LitStr;

pub struct Style {
    pub path: String,
    pub span: Span,
}

pub fn parse_styles(input: ParseStream, styles: &mut Vec<Style>, key: &syn::Ident) -> syn::Result<()> {
    if !styles.is_empty() {
        return Err(syn::Error::new(key.span(), "`style` is set twice; list every file once: `style = [\"a.css\", \"b.css\"]`"));
    }
    input.parse::<syn::Token![=]>()?;
    let literals: Vec<LitStr> = if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let list = content.parse_terminated(|input| input.parse::<LitStr>(), syn::Token![,]).map_err(|err| {
            syn::Error::new(err.span(), "`style` takes a file in `static/` like \"popup.css\", or a list of them")
        })?;
        if list.is_empty() {
            return Err(syn::Error::new(key.span(), "the `style` list is empty; leave `style` out instead"));
        }
        list.into_iter().collect()
    } else {
        vec![input.parse::<LitStr>().map_err(|err| {
            syn::Error::new(err.span(), "`style` takes a file in `static/` like \"popup.css\", or a list of them")
        })?]
    };
    for literal in literals {
        let path = literal.value();
        check_path(&path).map_err(|message| syn::Error::new(literal.span(), message))?;
        if styles.iter().any(|style| style.path == path) {
            return Err(syn::Error::new(literal.span(), format!("`{path}` is listed twice")));
        }
        styles.push(Style { path, span: literal.span() });
    }
    Ok(())
}

fn check_path(path: &str) -> Result<(), String> {
    if path.contains("://") || path.starts_with("data:") || path.starts_with("//") {
        return Err(format!("`{path}` isn't in your extension; download it into `static/`, so the page works offline and store reviewers can see everything it loads"));
    }
    if path.starts_with('/') || path.starts_with('\\') || path.contains(':') {
        return Err(format!("write the path inside `static/`, like \"popup.css\", not `{path}`"));
    }
    if path.split(['/', '\\']).any(|part| part == ".." || part == "." || part.is_empty()) {
        return Err(format!("`{path}` must be a plain path inside `static/`, without `.`, `..` or empty parts"));
    }
    if let Some(bad) = path.chars().find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))) {
        return Err(format!("`{path}` has `{bad}`; use letters, digits, `-`, `_`, `.` and `/` in stylesheet paths"));
    }
    if !path.ends_with(".css") {
        return Err(format!("`{path}` isn't a `.css` file"));
    }
    if path.starts_with("wasm/") || path.starts_with("_locales/") {
        return Err(format!("`{path}` is inside a folder oxichrome writes; put stylesheets elsewhere in `static/`"));
    }
    Ok(())
}

pub fn check_files(styles: &[Style]) -> syn::Result<TokenStream> {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let mut files = Vec::new();
    for style in styles {
        let file = Path::new(&root).join("static").join(&style.path);
        if !file.is_file() {
            return Err(syn::Error::new(style.span, format!("there's no `static/{}`; add it, or fix the path", style.path)));
        }
        files.push(file.to_string_lossy().into_owned());
    }
    Ok(quote! {
        #[doc(hidden)]
        const _: () = {
            #(const _: &[u8] = include_bytes!(#files);)*
        };
    })
}

pub fn paths(styles: &[Style]) -> Vec<String> {
    styles.iter().map(|style| style.path.clone()).collect()
}

pub fn parse_page_args(attr: TokenStream, page: &str) -> syn::Result<Vec<Style>> {
    let parser = |input: ParseStream| -> syn::Result<Vec<Style>> {
        let mut styles = Vec::new();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            match key.to_string().as_str() {
                "style" => parse_styles(input, &mut styles, &key)?,
                other => {
                    return Err(syn::Error::new(key.span(), format!("unknown {page} option `{other}`; the only option is `style = \"file.css\"`")))
                }
            }
            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }
        Ok(styles)
    };
    syn::parse::Parser::parse2(parser, attr)
}

#[cfg(test)]
mod tests {
    use super::check_path;

    #[test]
    fn style_paths() {
        assert!(check_path("popup.css").is_ok());
        assert!(check_path("css/base-2_x.min.css").is_ok());
        for bad in ["https://x.org/a.css", "//cdn/a.css", "data:text/css,a", "/popup.css", "../a.css", "a/../b.css", "./a.css", "a//b.css", "a b.css", "a.scss", "popup", "wasm/a.css", "_locales/a.css", "C:\\a.css", "a\\b.css"] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
    }
}
