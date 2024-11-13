use criterion::{black_box, criterion_group, criterion_main, Criterion};
use markcomp::mdast::document;
use markdown::ParseOptions;
use winnow::Parser;

pub fn parse(c: &mut Criterion) {
    let data = std::fs::read_to_string("../test-data/markdown.md").unwrap();

    c.bench_function("large parse", |b| {
        b.iter(|| black_box(document.parse(&data).unwrap()))
    });

    // let mut arena = markcomp::arena::NodeArena::new();
    // let doc = markcomp::arena::Document::parse(&data, &mut arena);
    // match doc {
    //     Ok(d) => {
    //         println!("document: {arena:#?}");
    //     }
    //     Err(e) => panic!("{e}"),
    // }

    c.bench_function("arena parse", |b| {
        b.iter(|| {
            let mut arena = markcomp::arena::NodeArena::new();
            let doc = markcomp::arena::Document::parse(&data, &mut arena).unwrap();

            black_box(doc)
        })
    });

    c.bench_function("lib parse", |b| {
        b.iter(|| black_box(markdown::to_mdast(&data, &ParseOptions::default())))
    });

    c.bench_function("comrak parse", |b| {
        b.iter(|| {
            let arena = comrak::Arena::new();
            let root = comrak::parse_document(&arena, &data, &comrak::Options::default());
            black_box(root);
        })
    });

    let data = std::fs::read_to_string("../test-data/small.md").unwrap();

    c.bench_function("small parse", |b| {
        b.iter(|| black_box(document.parse(&data).unwrap()))
    });

    c.bench_function("small arena parse", |b| {
        b.iter(|| {
            let mut arena = markcomp::arena::NodeArena::new();
            let doc = markcomp::arena::Document::parse(&data, &mut arena).unwrap();

            black_box(doc)
        })
    });

    c.bench_function("small lib parse", |b| {
        b.iter(|| black_box(markdown::to_mdast(&data, &ParseOptions::default())))
    });
}

fn write(c: &mut Criterion) {
    let data = std::fs::read_to_string("../test-data/markdown.md").unwrap();
    let parsed = document.parse(&data).unwrap();

    c.bench_function("large output", |b| {
        b.iter(|| {
            let mut output = Vec::new();
            for node in &parsed {
                node.write(&mut output).unwrap();
            }
            black_box(output);
        })
    });

    let mut arena = markcomp::arena::NodeArena::new();
    let doc = markcomp::arena::Document::parse(&data, &mut arena).unwrap();

    c.bench_function("arena output", |b| {
        b.iter(|| {
            let mut output = Vec::new();
            for node in doc.nodes.children(&arena) {
                node.write(&mut output, &arena).unwrap();
            }
            black_box(output);
        })
    });

    let data = std::fs::read_to_string("../test-data/small.md").unwrap();
    let parsed = document.parse(&data).unwrap();

    c.bench_function("small output", |b| {
        b.iter(|| {
            let mut output = Vec::new();
            for node in &parsed {
                node.write(&mut output).unwrap();
            }
            black_box(output);
        })
    });

    let mut arena = markcomp::arena::NodeArena::new();
    let doc = markcomp::arena::Document::parse(&data, &mut arena).unwrap();

    c.bench_function("small arena output", |b| {
        b.iter(|| {
            let mut output = Vec::new();
            for node in doc.nodes.children(&arena) {
                node.write(&mut output, &arena).unwrap();
            }
            black_box(output);
        })
    });
}

fn end_to_end(c: &mut Criterion) {
    let data = std::fs::read_to_string("../test-data/small.md").unwrap();

    c.bench_function("arena end to end", |b| {
        b.iter(|| {
            let mut arena = markcomp::arena::NodeArena::new();
            let doc = markcomp::arena::Document::parse(&data, &mut arena).unwrap();

            let mut output = Vec::new();
            for node in doc.nodes.children(&arena) {
                node.write(&mut output, &arena).unwrap();
            }
            black_box(output);
        })
    });

    let raw = data.as_bytes();
    c.bench_function("visitor end to end", |b| {
        b.iter(|| {
            let visit = markcomp::visitor::SimpleVisitor::new(raw).unwrap();
            black_box(visit.output());
        })
    });

    c.bench_function("markdown end to end", |b| {
        b.iter(|| {
            black_box(markdown::to_html(&data));
        })
    });

    c.bench_function("comrak end to end", |b| {
        b.iter(|| {
            black_box(comrak::markdown_to_html(&data, &comrak::Options::default()));
        })
    });

    let data = std::fs::read_to_string("../test-data/markdown.md").unwrap();

    c.bench_function("arena large end to end", |b| {
        b.iter(|| {
            let mut arena = markcomp::arena::NodeArena::new();
            let doc = markcomp::arena::Document::parse(&data, &mut arena).unwrap();

            let mut output = Vec::new();
            for node in doc.nodes.children(&arena) {
                node.write(&mut output, &arena).unwrap();
            }
            black_box(output);
        })
    });

    let raw = data.as_bytes();
    c.bench_function("visitor large end to end", |b| {
        b.iter(|| {
            let visit = markcomp::visitor::SimpleVisitor::new(raw).unwrap();
            black_box(visit.output());
        })
    });

    c.bench_function("markdown large end to end", |b| {
        b.iter(|| {
            black_box(markdown::to_html(&data));
        })
    });

    c.bench_function("comrak large end to end", |b| {
        b.iter(|| {
            black_box(comrak::markdown_to_html(&data, &comrak::Options::default()));
        })
    });
}

criterion_group!(benches, parse, write, end_to_end);
criterion_main!(benches);
