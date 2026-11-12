pub mod extension;
pub mod background;
pub mod event_handler;
pub mod page;
pub mod popup;
pub mod options_page;
pub mod side_panel;
pub mod omnibox;
pub mod content_script;
pub mod offscreen;
pub mod menu;
pub mod command;

pub struct Order {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

pub fn export_name(kind: &str, name: &syn::Ident, order: &Order) -> syn::Ident {
    let place = format!("{}:{}:{}", order.file, order.line, order.column);
    let hash = place.bytes().fold(0x811c_9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    syn::Ident::new(&format!("__oxichrome_{kind}_{name}_{hash:08x}"), name.span())
}
