use proc_macro::TokenStream;

mod codegen;
mod error;
mod events;
mod i18n;
mod metadata;
mod parse;
mod permission;
mod shortcut;

/// ```ignore
/// #[oxichrome::extension(
///     name = "My Extension",
///     version = "1.0.0",
///     permissions = [Storage, Tabs]
/// )]
/// struct MyExtension;
/// ```
#[proc_macro_attribute]
pub fn extension(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::extension::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn background(_attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::background::expand(item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::on(runtime::on_installed)]
/// async fn handle_install(details: JsValue) { }
/// ```
#[proc_macro_attribute]
pub fn on(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::event_handler::expand(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::popup]
/// fn Popup() -> impl IntoView {
///     view! { <p>"Hello from the popup!"</p> }
/// }
/// ```
#[proc_macro_attribute]
pub fn popup(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::popup::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::options_page]
/// fn Options() -> impl IntoView {
///     view! { <h1>"Settings"</h1> }
/// }
/// ```
#[proc_macro_attribute]
pub fn options_page(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::options_page::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::side_panel(title = "Notes", open_on_action_click)]
/// fn Panel() -> impl IntoView {
///     view! { <h1>"Notes"</h1> }
/// }
/// ```
#[proc_macro_attribute]
pub fn side_panel(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::side_panel::expand(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::omnibox(keyword = "docs", default = "Search the docs for {input}")]
/// async fn suggest(input: String) -> Vec<omnibox::Suggestion> {
///     vec![omnibox::Suggestion::new("https://docs.rs", "Open docs.rs")]
/// }
/// ```
#[proc_macro_attribute]
pub fn omnibox(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::omnibox::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::content_script(matches = ["<all_urls>"])]
/// async fn inject() {
///     // runs in web page context
/// }
/// ```
#[proc_macro_attribute]
pub fn content_script(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::content_script::expand(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::offscreen(reasons = [DomParser], justification = "Read the HTML of saved pages")]
/// async fn title(html: String) -> String {
///     // runs in an offscreen document, with a DOM
/// }
/// ```
#[proc_macro_attribute]
pub fn offscreen(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::offscreen::expand(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// let text: String = oxichrome::t!(greeting, name);
/// ```
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let order = order();
    i18n::expand_t(input.into(), &format!("{}:{}:{}", order.file, order.line, order.column))
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

fn order() -> codegen::Order {
    let span = proc_macro::Span::call_site();
    codegen::Order {
        file: span.file(),
        line: span.line(),
        column: span.column(),
    }
}

/// ```ignore
/// #[oxichrome::menu(title = "Search for \"%s\"", contexts = [selection])]
/// async fn search(click: context_menus::Click) { }
/// ```
#[proc_macro_attribute]
pub fn menu(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::menu::expand_menu(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::submenu(title = "Copy as", contexts = [page, link])]
/// struct CopyAs;
/// ```
#[proc_macro_attribute]
pub fn submenu(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::menu::expand_submenu(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::command(description = "Toggle dark mode", shortcut = "Alt+Shift+D")]
/// async fn toggle_dark(press: commands::Press) { }
/// ```
#[proc_macro_attribute]
pub fn command(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::command::expand(attr.into(), item.into(), order())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}
