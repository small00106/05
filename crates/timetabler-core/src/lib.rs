//! timetabler-core：排课核心 crate（纯计算，无 IO / HTTP）。
//!
//! 本阶段内容：
//! - [`bitmap`]：时间槽占用位图（`Vec<u64>` 实现，不用 `HashMap` 硬查）。
//! - [`model`]：教师、班级、科目、教室、周课时需求、节次表、课表等数据模型。
//! - [`integrity`]：输入（Problem）自身的引用完整性检查。
//! - [`validate`]：硬约束校验器，给定完整课表报出所有违反的硬约束。
//! - [`testgen`]：确定性测试数据生成器（20 个班、60 名教师），供测试与基准使用。

pub mod bitmap;
pub mod integrity;
pub mod model;
pub mod testgen;
pub mod validate;

pub use integrity::IntegrityError;
pub use model::{
    Class, ClassId, Lesson, Problem, Requirement, RequirementId, Room, RoomId, Slot, Subject,
    SubjectId, Teacher, TeacherId, Timetable, WeekSchedule,
};
pub use validate::{validate, Violation, ViolationKind};
