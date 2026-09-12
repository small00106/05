# timetabler

排课系统。当前阶段为纯计算的 core crate（无 HTTP / IO）：数据模型、位图时间槽、硬约束校验器。

## 布局

- `crates/timetabler-core`
  - `src/bitmap.rs` — 定长位图（`Vec<u64>`），时间槽占用标记，不用 `HashMap`
  - `src/model.rs` — 教师 / 班级 / 科目 / 教室 / 周课时需求 / 节次表 / 课表
  - `src/validate.rs` — 硬约束校验器：给定完整课表，报出所有违反（谁、哪个时间槽、哪条约束）
  - `src/testgen.rs` — 确定性测试数据生成器（20 个班、60 名教师、700 节课）
  - `tests/validator.rs` — 校验器集成测试
  - `benches/validate.rs` — criterion 基准：一次全量校验的耗时

## 硬约束

1. 同一教师 / 班级 / 教室在同一时间槽只能有一节课
2. 教师不可用时段必须避开
3. 连堂（如实验课）两节必须相邻且不跨中午
4. 每个班每天同一科目不超过两节
5. 各需求实际排课节数等于周课时（完整性）

## 常用命令

```sh
cargo test --workspace    # 单元 + 集成测试
cargo bench --workspace   # 全量校验基准（当前约 21 µs / 700 节课）
```

工具链：Rust 1.79。dev 依赖中 clap / half / rayon 已钉在兼容 1.79 的版本（见 Cargo.lock）。
