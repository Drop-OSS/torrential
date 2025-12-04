use std::{thread::sleep, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use tokio::runtime::Runtime;

use crate::benchmark_server::{CHUNK_SIZE, TOTAL_BYTES};

mod benchmark_server;

const SERVER_URL: &str = "http://127.0.0.1:5000";
const BENCHMARK_CHUNK_URL: &str = "/api/v1/depot/test-game/v1/chunk_";
const BENCHMARK_TOKEN: &str = "test-token-for-benchmark";

#[derive(Serialize)]
pub struct TokenPayload {
    token: String,
}

// The benchmark function setup
fn benchmark(c: &mut Criterion) {
    std::thread::spawn(|| {
        let server_rt = Runtime::new().unwrap();
        server_rt.block_on(async {
            benchmark_server::start().await;
        })
    });

    // Ensure that the server is properly started up + some wiggle room
    // If it's too short the client won't start up in time
    sleep(Duration::from_millis(1));

    let client = Client::new();

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        client
            .post(format!("{}/token", SERVER_URL))
            .json(&TokenPayload {
                token: String::from(BENCHMARK_TOKEN),
            })
            .send()
            .await
            .unwrap();
    });

    let mut group = c.benchmark_group("Torrential Serve File");

    group.throughput(criterion::Throughput::Bytes(TOTAL_BYTES as u64));

    group.sample_size(500);
    let rt = Runtime::new().unwrap();

    group.bench_function("serve", move |b| {
        b.to_async(&rt).iter_batched(
            || client.clone(),
            |client| async move {
                for chunk in 0..(TOTAL_BYTES / CHUNK_SIZE)  {
                    let url = format!("{}{}{}", SERVER_URL, BENCHMARK_CHUNK_URL, chunk);
                    let resp = client.get(url).send().await.unwrap();
                    assert_eq!(resp.status(), StatusCode::OK);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// Grouping your benchmarks
criterion_group!(benches, benchmark);
criterion_main!(benches);
