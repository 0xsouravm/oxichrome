use proc_macro2::{Literal, TokenStream};
use quote::quote;

pub const SECTION: &str = "oxichrome.v1";

pub fn emit(entry: serde_json::Value) -> TokenStream {
    let mut bytes = entry.to_string().into_bytes();
    bytes.push(b'\n');
    let len = bytes.len();
    let data = Literal::byte_string(&bytes);
    quote! {
        #[cfg(target_arch = "wasm32")]
        #[doc(hidden)]
        const _: () = {
            #[unsafe(link_section = #SECTION)]
            #[used]
            static OXICHROME_METADATA: [u8; #len] = *#data;
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_one_json_line() {
        let tokens = emit(serde_json::json!({ "kind": "popup", "fn_name": "Line\nBreak" })).to_string();
        assert!(tokens.contains("oxichrome.v1"), "{tokens}");
        assert!(
            tokens.contains(r#"b"{\"fn_name\":\"Line\\nBreak\",\"kind\":\"popup\"}\n""#),
            "{tokens}"
        );
    }
}
