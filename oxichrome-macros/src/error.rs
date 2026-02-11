use proc_macro2::Span;

pub enum MacroError {
    MissingArgument { span: Span, name: &'static str },
    UnknownArgument { span: Span, name: String },
    InvalidValue { span: Span, message: String },
    InvalidTarget { span: Span, message: String },
    Syn(syn::Error),
}

impl MacroError {
    pub fn into_compile_error(self) -> proc_macro2::TokenStream {
        let err: syn::Error = self.into();
        err.into_compile_error()
    }
}

impl From<syn::Error> for MacroError {
    fn from(err: syn::Error) -> Self {
        MacroError::Syn(err)
    }
}

impl From<MacroError> for syn::Error {
    fn from(err: MacroError) -> Self {
        match err {
            MacroError::MissingArgument { span, name } => {
                syn::Error::new(span, format!("missing required argument `{name}`"))
            }
            MacroError::UnknownArgument { span, name } => {
                syn::Error::new(span, format!("unknown argument `{name}`"))
            }
            MacroError::InvalidValue { span, message } => {
                syn::Error::new(span, message)
            }
            MacroError::InvalidTarget { span, message } => {
                syn::Error::new(span, message)
            }
            MacroError::Syn(err) => err,
        }
    }
}
