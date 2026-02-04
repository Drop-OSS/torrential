use tokio::sync::Semaphore;
pub mod download;
pub mod serve;
pub mod handlers;
pub mod state;
pub mod util;
pub mod proto;
pub mod conversions;
pub mod server;

pub use download::DownloadContext;

static GLOBAL_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);
