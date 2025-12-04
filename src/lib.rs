use tokio::sync::Semaphore;
mod download;
pub mod serve;
pub mod handlers;
mod manifest;
mod remote;
pub mod state;
mod token;
mod util;
pub mod void;
#[cfg(test)]
mod tests;

pub use download::DownloadContext;
pub use download::{BackendFactory, DropBackendFactory};
pub use manifest::DropChunk;
pub use remote::LibrarySource;
pub use remote::{
    ContextProvider, ContextResponseBody, DropContextProvider, DropLibraryProvider, LibraryBackend,
    LibraryConfigurationProvider,
};
pub use token::{TokenPayload, set_token};
pub use util::ErrorOption;

static GLOBAL_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);
