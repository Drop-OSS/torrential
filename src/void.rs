use std::{
    collections::HashMap,
    io::SeekFrom,
    pin::Pin,
    task::{Context, Poll},
};

use async_trait::async_trait;
use droplet_rs::versions::types::{MinimumFileObject, VersionBackend, VersionFile};
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};
use crate::{
    BackendFactory, ContextProvider, ContextResponseBody, DropChunk, ErrorOption, LibraryBackend, LibraryConfigurationProvider, LibrarySource, state::AppInitData
};

pub struct VoidReader {
    size: u64,
    cursor: u64,
}
impl AsyncRead for VoidReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let remaining = self.size.saturating_sub(self.cursor);
        // dbg!(remaining);
        if remaining == 0 {
            // End of void file
            buf.set_filled(0);
            return Poll::Ready(Ok(()));
        }

        let to_fill = remaining.min(buf.remaining() as u64) as usize;

        buf.initialize_unfilled().fill(0);

        buf.advance(to_fill);

        self.cursor += to_fill as u64;

        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for VoidReader {
    fn start_seek(mut self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
        let new_cursor = match position {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(offset) => (self.size as i64 + offset).max(0) as u64,
            SeekFrom::Current(offset) => (self.cursor as i64 + offset).max(0) as u64,
        };
        self.cursor = new_cursor.min(self.size);
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor))
    }
}

#[derive(Clone)]
pub struct VoidBackend {
    num_files: usize,
    file_size: u64,
}
impl VoidBackend {
    pub fn new(num_files: usize, file_size: u64) -> Self {
        Self {
            num_files,
            file_size,
        }
    }
}

#[async_trait]
impl VersionBackend for VoidBackend {
    fn require_whole_files(&self) -> bool {
        false
    }
    async fn list_files(&mut self) -> anyhow::Result<Vec<VersionFile>> {
        println!("Files listed");
        Ok((0..self.num_files)
            .map(|idx| VersionFile {
                relative_filename: idx.to_string(),
                permission: 0,
                size: self.file_size,
            })
            .collect())
    }
    async fn peek_file(&mut self, sub_path: String) -> anyhow::Result<VersionFile> {
        println!("File peeked");
        Ok(VersionFile {
            relative_filename: sub_path,
            permission: 0,
            size: self.file_size,
        })
    }
    async fn reader(
        &mut self,
        _file: &VersionFile,
        start: u64,
        end: u64,
    ) -> anyhow::Result<Box<dyn MinimumFileObject>> {
        Ok(Box::new(VoidReader {
            size: end,
            cursor: start,
        }))
    }
}

pub struct VoidBackendFactory {
    pub num_files: usize,
    pub file_size: u64,
}
impl VoidBackendFactory {
    pub fn new(num_files: usize, file_size: u64) -> Self {
        Self {
            num_files,
            file_size,
        }
    }
}

impl BackendFactory for VoidBackendFactory {
    fn create_backend(
        &self,
        _init_data: &AppInitData,
        _context: &ContextResponseBody,
        _version_name: &String,
    ) -> Result<Box<dyn VersionBackend + Send + Sync>, reqwest::StatusCode> {
        Ok(Box::new(VoidBackend::new(self.num_files, self.file_size)))
    }
}

pub struct VoidContextProvider {
    pub total_bytes: usize,
    pub chunk_size: usize,
    pub file_size: u64,
}

impl VoidContextProvider {
    pub fn new(total_bytes: usize, chunk_size: usize, file_size: u64) -> Self {
        Self {
            total_bytes,
            chunk_size,
            file_size,
        }
    }
}

#[async_trait]
impl ContextProvider for VoidContextProvider {
    async fn fetch_context(
        &self,
        _token: String,
        _game_id: String,
        _version_name: String,
    ) -> Result<ContextResponseBody, ErrorOption> {
        let mut remaining_bytes = self.total_bytes;
        let mut manifest = HashMap::new();
        let mut file_id_counter = 0;
        println!("Context provided");

        const FILENAME: &str = "large.bin";

        while remaining_bytes > 0 {
            let length = remaining_bytes.min(self.chunk_size);

            manifest
                .entry(FILENAME.to_string())
                .or_insert_with(|| DropChunk {
                    ids: Vec::new(),
                    lengths: Vec::new(),
                })
                .ids
                .push(format!("chunk_{file_id_counter}"));

            manifest.get_mut(FILENAME).unwrap().lengths.push(length);

            remaining_bytes -= length;
            file_id_counter += 1;
        }

        Ok(ContextResponseBody {
            manifest,
            library_id: String::from("void"),
            library_path: String::from("."),
        })
    }
}

pub struct VoidLibraryProvider;

#[async_trait]
impl LibraryConfigurationProvider for VoidLibraryProvider {
    async fn fetch_sources(&self, _token: &String) -> anyhow::Result<Vec<LibrarySource>> {
        Ok(vec![LibrarySource {
            options: serde_json::json!({
                "baseDir": "/tmp/void_dir"
            }),
            id: String::from("Void"),
            backend: LibraryBackend::Filesystem,
        }])
    }
}
