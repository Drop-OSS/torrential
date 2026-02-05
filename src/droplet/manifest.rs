use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use log::{info, warn};
use protobuf::Message;
use serde_json::json;
use tokio::{spawn, sync::Semaphore};

use crate::{
    droplet,
    proto::{
        core::{DropBoundType, TorrentialBound},
        droplet::{
            GenerateManifest, ManifestComplete, ManifestError, ManifestLog, ManifestProgress,
        },
    },
    server::DropServer,
};

static READER_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| {
    let cores = num_cpus::get();
    Semaphore::new(cores)
});

async fn generate_manifest_raw(
    server: Arc<DropServer>,
    message: TorrentialBound,
) -> Result<(), anyhow::Error> {
    let manifest_message = GenerateManifest::parse_from_bytes(&message.data)?;
    info!(
        "seven zip install: {}",
        *droplet_rs::versions::backends::SEVEN_ZIP_INSTALLED
    );

    let manifest = droplet_rs::manifest::generate_manifest_rusty(
        &PathBuf::from(manifest_message.version_dir),
        |progress| {
            let mut progress_message = ManifestProgress::new();
            progress_message.progress = progress;

            let server = server.clone();
            let message_id = message.message_id.clone();
            spawn(async move {
                let _ = server
                    .send_message(
                        DropBoundType::MANIFEST_PROGRESS,
                        progress_message,
                        Some(message_id),
                    )
                    .await;
            });
        },
        |log_line| {
            let mut progress_log = ManifestLog::new();
            progress_log.log_line = log_line;

            let server = server.clone();
            let message_id = message.message_id.clone();
            spawn(async move {
                let _ = server
                    .send_message(DropBoundType::MANIFEST_LOG, progress_log, Some(message_id))
                    .await;
            });
        },
        Some(&READER_SEMAPHORE),
    )
    .await?;

    let mut manifest_complete = ManifestComplete::new();
    manifest_complete.manifest = json!(manifest).to_string();

    server
        .send_message(
            DropBoundType::MANIFEST_COMPLETE,
            manifest_complete,
            Some(message.message_id),
        )
        .await?;

    Ok(())
}

pub async fn generate_manifest(server: Arc<DropServer>, message: TorrentialBound) {
    let message_id = message.message_id.clone();
    warn!("generating manifest...");
    let result = generate_manifest_raw(server.clone(), message).await;
    info!("manifest generation exited");
    if let Err(err) = result {
        warn!("manifest generation failed with err: {:?}", err);
        let mut manifest_err = ManifestError::new();
        manifest_err.error = err.to_string();
        let _ = server
            .send_message(
                DropBoundType::MANIFEST_ERROR,
                manifest_err,
                Some(message_id),
            )
            .await
            .inspect_err(|err| {
                warn!("failed to send manifest err: {err:?}");
            });
    }
}
