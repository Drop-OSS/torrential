use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use protobuf::Message;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt as _, BufReader},
    net::{
        TcpListener,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    spawn,
};
use waitmap::WaitMap;

use crate::proto::core::Response;

pub mod download;

pub struct DropServer {
    write_stream: Mutex<OwnedWriteHalf>,
    waitmap: WaitMap<String, Response>,
}

impl DropServer {
    async fn recieve_subroutine(myself: Arc<DropServer>, read_stream: OwnedReadHalf) -> ! {
        let mut buffered_reader = BufReader::new(read_stream);

        loop {
            let mut length_buffer: [u8; 8] = [0; 8];
            buffered_reader
                .read_exact(&mut length_buffer)
                .await
                .expect("failed to read from internal pipe");

            let length = usize::from_le_bytes(length_buffer);
            let mut buffer = Vec::with_capacity(length);

            buffered_reader
                .read_exact(&mut buffer)
                .await
                .expect("failed to read from internal pipe");

            let message =
                Response::parse_from_bytes(&buffer).expect("response didn't deserialize correctly");
            myself.waitmap.insert(message.message_id.clone(), message);
        }
    }

    async fn wait_for_message_id<T>(&self, message_id: &str) -> Result<T, anyhow::Error>
    where
        T: protobuf::Message,
    {
        let message = self
            .waitmap
            .wait(message_id.clone())
            .await
            .ok_or(anyhow!("no response returned for value"))?;

        let message = message.value();

        match message.type_.unwrap() {
            crate::proto::core::ResponseType::ERROR => {
                return Err(anyhow!(String::from_utf8(message.data.clone()).unwrap()));
            }
            _ => {
                let response = T::parse_from_bytes(&message.data)?;
                return Ok(response);
            }
        }
    }

    async fn send_message<T>(&self, message: T) -> Result<(), anyhow::Error>
    where
        T: protobuf::Message,
    {
        let mut buf = Vec::new();
        message.write_to_vec(&mut buf)?;

        {
            let mut mutex_lock = self
                .write_stream
                .lock()
                .expect("failed to lock send stream");
            mutex_lock.write(&buf.len().to_le_bytes()).await?;
            mutex_lock.write_all(&buf).await?;
        };

        Ok(())
    }
}

pub async fn create_drop_server() -> Result<Arc<DropServer>, anyhow::Error> {
    let server = TcpListener::bind("127.0.0.1:33148").await?;

    let (drop_stream, _) = server.accept().await?;

    let (read, write) = drop_stream.into_split();

    let client = Arc::new(DropServer {
        write_stream: Mutex::new(write),
        waitmap: WaitMap::new(),
    });

    spawn(DropServer::recieve_subroutine(client.clone(), read));

    Ok(client)
}
