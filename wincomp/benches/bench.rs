use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wincomp::Document;

pub fn criterion_benchmark(c: &mut Criterion) {
    let data = std::fs::read_to_string("../test-data/large.html").unwrap();
    c.bench_function("large parse", |b| {
        b.iter(|| black_box(Document::new(&data).unwrap()))
    });

    let data = std::fs::read_to_string("../test-data/small.html").unwrap();
    c.bench_function("small parse", |b| {
        b.iter(|| black_box(Document::new(&data).unwrap()))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
