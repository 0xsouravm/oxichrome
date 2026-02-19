use proc_macro::TokenStream;

mod error;
mod parse;
mod codegen;

/// ```ignore
/// #[oxichrome::extension(
///     name = "My Extension",
///     version = "1.0.0",
///     permissions = ["storage", "tabs"]
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
    codegen::background::expand(item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// ```ignore
/// #[oxichrome::on(runtime::on_installed)]
/// async fn handle_install(details: JsValue) { }
/// ```
#[proc_macro_attribute]
pub fn on(attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::event_handler::expand(attr.into(), item.into())
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
pub fn popup(_attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::popup::expand(item.into())
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
pub fn options_page(_attr: TokenStream, item: TokenStream) -> TokenStream {
    codegen::options_page::expand(item.into())
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
    codegen::content_script::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}
