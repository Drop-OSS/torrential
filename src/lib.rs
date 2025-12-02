use tokio::sync::Semaphore;
mod download;
pub mod handlers;
mod manifest;
mod remote;
pub mod state;
mod token;
mod util;

pub use download::DownloadContext;
pub use download::{BackendFactory, DropBackendFactory};
pub use remote::{
    DropLibraryProvider, DropContextProvider, LibraryConfigurationProvider, ContextProvider,
};
pub use token::set_token;

static GLOBAL_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);
