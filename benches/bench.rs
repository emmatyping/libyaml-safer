use std::mem::MaybeUninit;

use criterion::{criterion_group, criterion_main, Criterion};
use libyaml_safer::{Document, Emitter, Parser};
use unsafe_libyaml::*;

static VERY_LARGE_YAML: &[u8] = include_bytes!("very_large.yml");

pub fn parser(c: &mut Criterion) {
    c.bench_function("libyaml-safer parse large", |b| {
        // Note: Not using `iter_with_large_drop` because that would be unfair
        // to unsafe-libyaml, which needs a call to `yaml_document_delete`.
        b.iter(|| {
            let mut input = VERY_LARGE_YAML;
            let mut parser = Parser::new();
            parser.set_input(&mut input);
            Document::load(&mut parser)
        })
    });

    c.bench_function("unsafe-libyaml parse large", |b| {
        b.iter(|| unsafe {
            let mut parser = MaybeUninit::zeroed();
            if !yaml_parser_initialize(parser.as_mut_ptr()).ok {
                panic!("yaml_parser_initialize failed");
            }
            let mut parser = parser.assume_init();
            yaml_parser_set_input_string(
                &mut parser,
                VERY_LARGE_YAML.as_ptr(),
                VERY_LARGE_YAML.len() as _,
            );
            let mut document = MaybeUninit::zeroed();
            if !yaml_parser_load(&mut parser, document.as_mut_ptr()).ok {
                panic!("yaml_parser_load faled");
            };
            yaml_document_delete(document.as_mut_ptr());
            yaml_parser_delete(&mut parser);
        })
    });

    c.bench_function("libyaml-safer emit large", |b| {
        // output shouldn't be much larger than the input, but just to be safe...
        let mut buffer = Vec::with_capacity(VERY_LARGE_YAML.len());

        let doc = {
            let mut parser = Parser::new();
            let mut input = VERY_LARGE_YAML;
            parser.set_input(&mut input);
            Document::load(&mut parser).unwrap()
        };

        b.iter_custom(|iters| {
            let mut measurement = std::time::Duration::ZERO;
            for _ in 0..iters {
                let doc = doc.clone();
                let start_time = std::time::Instant::now();
                let mut emitter = Emitter::new();
                emitter.set_output(&mut buffer);
                doc.dump(&mut emitter).unwrap();
                measurement += start_time.elapsed();
            }
            measurement
        });
    });

    c.bench_function("unsafe-libyaml emit large", |b| {
        // output shouldn't be much larger than the input, but just to be safe...
        let mut buffer = vec![0; VERY_LARGE_YAML.len() * 2];

        // `yaml_document_t` cannot be cloned, so we have to parse it every iteration unfortunately.
        let read_doc = || unsafe {
            let mut parser = MaybeUninit::zeroed();
            if !yaml_parser_initialize(parser.as_mut_ptr()).ok {
                panic!("yaml_parser_initialize failed");
            }
            let mut parser = parser.assume_init();
            yaml_parser_set_input_string(
                &mut parser,
                VERY_LARGE_YAML.as_ptr(),
                VERY_LARGE_YAML.len() as _,
            );
            let mut document = MaybeUninit::zeroed();
            if !yaml_parser_load(&mut parser, document.as_mut_ptr()).ok {
                panic!("yaml_parser_load faled");
            };
            yaml_parser_delete(&mut parser);
            document.assume_init()
        };

        b.iter_custom(|iters| {
            let mut measurement = std::time::Duration::ZERO;
            for _ in 0..iters {
                unsafe {
                    let mut doc = read_doc();
                    let start_time = std::time::Instant::now();
                    let mut emitter = MaybeUninit::zeroed();
                    if !yaml_emitter_initialize(emitter.as_mut_ptr()).ok {
                        panic!("yaml_emitter_initialize failed");
                    }
                    let mut emitter = emitter.assume_init();
                    let mut size_written = 0;
                    yaml_emitter_set_output_string(
                        &mut emitter,
                        buffer.as_mut_ptr(),
                        buffer.len() as _,
                        &mut size_written,
                    );
                    if !yaml_emitter_dump(&mut emitter, &mut doc).ok {
                        panic!("yaml_emitter_dump failed");
                    }
                    measurement += start_time.elapsed();
                    yaml_emitter_delete(&mut emitter);
                }
            }
            measurement
        });
    });
}

criterion_group!(benches, parser);
criterion_main!(benches);
