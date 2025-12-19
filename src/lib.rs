use tokio::sync::Semaphore;
mod download;
pub mod serve;
pub mod handlers;
mod remote;
pub mod state;
mod token;
mod util;

pub use download::DownloadContext;
pub use token::set_token;

static GLOBAL_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);
