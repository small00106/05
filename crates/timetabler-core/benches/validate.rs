//! 基准：对 20 班 / 60 教师 / 700 节课的完整课表做一次全量硬约束校验。

use criterion::{criterion_group, criterion_main, Criterion};
use timetabler_core::{testgen, validate};

fn bench_validate_full(c: &mut Criterion) {
    let (problem, timetable) = testgen::generate(42);
    c.bench_function("validate_full_20classes_60teachers", |b| {
        b.iter(|| validate(std::hint::black_box(&problem), std::hint::black_box(&timetable)))
    });
}

criterion_group!(benches, bench_validate_full);
criterion_main!(benches);
