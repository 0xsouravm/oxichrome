pub mod source_parser;
pub mod manifest;
pub mod shims;
pub mod templates;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Browser {
    #[default]
    Chromium,
    Firefox,
}
