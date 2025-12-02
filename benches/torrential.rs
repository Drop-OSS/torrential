use criterion::{criterion_group, criterion_main, Criterion};

fn torrential() {
    
}

// The benchmark function setup
fn benchmark(c: &mut Criterion) {
    c.bench_function("fibonacci 20", |b| b.iter(|| torrential()));
}

// Grouping your benchmarks
criterion_group!(benches, benchmark);
criterion_main!(benches);