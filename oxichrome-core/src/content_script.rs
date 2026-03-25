/// When to inject the content script into the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunAt {
    /// Inject after the DOM is complete, but before subresources like images have loaded.
    DocumentEnd,
    /// Inject after the page is idle (default).
    DocumentIdle,
    /// Inject before any other DOM content is constructed or any script is run.
    DocumentStart,
}
