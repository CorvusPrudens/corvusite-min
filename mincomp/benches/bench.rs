use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mincomp::Dom;

pub fn criterion_benchmark(c: &mut Criterion) {
    let data = std::fs::read_to_string("../test-data/large.html").unwrap();
    c.bench_function("large parse", |b| {
        b.iter(|| {
            let mut cursor = std::io::Cursor::new(data.as_bytes());
            black_box(Dom::new(&mut cursor).unwrap())
        })
    });

    let data = std::fs::read_to_string("../test-data/small.html").unwrap();
    c.bench_function("small parse", |b| {
        b.iter(|| {
            let mut cursor = std::io::Cursor::new(data.as_bytes());
            black_box(Dom::new(&mut cursor).unwrap())
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
