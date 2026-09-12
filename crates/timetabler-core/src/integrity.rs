//! 输入（[`Problem`]）自身的引用完整性检查。
//!
//! 课表校验的前置条件：Problem 里的所有交叉引用必须合法。`validate` 会先跑这套检查，
//! 有任何问题就只报这些、不再检查课表（在残缺输入上继续查只会产生级联误报）；
//! 也可以在装载输入后主动调用 [`Problem::check_integrity`] 提前发现数据错误。
//!
//! 检查项：需求的 班级/科目/教师/指定教室 引用、班级本班教室、教师不可用位图长度、
//! 需求 id 与位置一致（校验器按下标寻址需求）、节次表参数合法。

use crate::model::*;

/// 输入数据的完整性错误（引用越界 / 形状不符）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrityError {
    /// 节次表参数非法（字段是 pub 的，可能绕过 `WeekSchedule::new` 的断言构造）。
    InvalidSchedule {
        days: u8,
        periods_per_day: u8,
        lunch_after_period: u8,
    },
    /// 需求的 id 与其在 `requirements` 中的位置不一致（校验器按 id 当下标寻址）。
    RequirementIdMismatch { position: u32, id: u32 },
    /// 需求引用了不存在的班级。
    RequirementClassOutOfRange { requirement: RequirementId, class: u32 },
    /// 需求引用了不存在的科目。
    RequirementSubjectOutOfRange { requirement: RequirementId, subject: u32 },
    /// 需求引用了不存在的教师。
    RequirementTeacherOutOfRange { requirement: RequirementId, teacher: u32 },
    /// 需求指定的教室不存在。
    RequirementRoomOutOfRange { requirement: RequirementId, room: u32 },
    /// 班级的本班教室不存在。
    ClassHomeRoomOutOfRange { class: ClassId, room: u32 },
    /// 教师不可用位图长度与一周槽数不符。
    TeacherAvailabilityLenMismatch {
        teacher: TeacherId,
        expected: usize,
        actual: usize,
    },
}

impl IntegrityError {
    /// 生成中文描述。
    pub fn describe(&self, p: &Problem) -> String {
        match self {
            IntegrityError::InvalidSchedule {
                days,
                periods_per_day,
                lunch_after_period,
            } => format!(
                "节次表参数非法：每周 {days} 天、每天 {periods_per_day} 节、午休位于第 {lunch_after_period} 节后"
            ),
            IntegrityError::RequirementIdMismatch { position, id } => format!(
                "需求 id 与位置不符：第 {position} 个需求的 id 为 #{id}（校验器按下标寻址，二者必须一致）"
            ),
            IntegrityError::RequirementClassOutOfRange { requirement, class } => format!(
                "需求{} 引用了不存在的班级 #{class}（共 {} 个班）",
                requirement,
                p.classes.len()
            ),
            IntegrityError::RequirementSubjectOutOfRange { requirement, subject } => format!(
                "需求{} 引用了不存在的科目 #{subject}（共 {} 门）",
                requirement,
                p.subjects.len()
            ),
            IntegrityError::RequirementTeacherOutOfRange {
                requirement,
                teacher,
            } => format!(
                "需求{} 引用了不存在的教师 #{teacher}（共 {} 名）",
                requirement,
                p.teachers.len()
            ),
            IntegrityError::RequirementRoomOutOfRange { requirement, room } => format!(
                "需求{} 指定了不存在的教室 #{room}（共 {} 间）",
                requirement,
                p.rooms.len()
            ),
            IntegrityError::ClassHomeRoomOutOfRange { class, room } => format!(
                "{} 的本班教室指向不存在的教室 #{room}（共 {} 间）",
                p.class_name(*class),
                p.rooms.len()
            ),
            IntegrityError::TeacherAvailabilityLenMismatch {
                teacher,
                expected,
                actual,
            } => format!(
                "{} 的不可用位图长度为 {actual}，应等于一周槽数 {expected}",
                p.teacher_name(*teacher)
            ),
        }
    }
}

impl Problem {
    /// 检查输入自身的引用完整性，返回全部发现的问题（空 Vec 表示干净）。
    ///
    /// 只读长度与取值做比较，自身不会 panic。
    pub fn check_integrity(&self) -> Vec<IntegrityError> {
        let mut errors = Vec::new();

        // 节次表是其他检查的基础，非法时直接报这一项。
        let s = &self.schedule;
        if s.days == 0
            || s.days > 7
            || s.periods_per_day == 0
            || s.lunch_after_period > s.periods_per_day
        {
            return vec![IntegrityError::InvalidSchedule {
                days: s.days,
                periods_per_day: s.periods_per_day,
                lunch_after_period: s.lunch_after_period,
            }];
        }

        let nslots = self.slot_count();
        for (i, t) in self.teachers.iter().enumerate() {
            if t.unavailable.len() != nslots {
                errors.push(IntegrityError::TeacherAvailabilityLenMismatch {
                    teacher: TeacherId(i as u32),
                    expected: nslots,
                    actual: t.unavailable.len(),
                });
            }
        }
        for (i, c) in self.classes.iter().enumerate() {
            if c.home_room.idx() >= self.rooms.len() {
                errors.push(IntegrityError::ClassHomeRoomOutOfRange {
                    class: ClassId(i as u32),
                    room: c.home_room.0,
                });
            }
        }
        for (i, r) in self.requirements.iter().enumerate() {
            if r.id.idx() != i {
                errors.push(IntegrityError::RequirementIdMismatch {
                    position: i as u32,
                    id: r.id.0,
                });
            }
            if r.class.idx() >= self.classes.len() {
                errors.push(IntegrityError::RequirementClassOutOfRange {
                    requirement: r.id,
                    class: r.class.0,
                });
            }
            if r.subject.idx() >= self.subjects.len() {
                errors.push(IntegrityError::RequirementSubjectOutOfRange {
                    requirement: r.id,
                    subject: r.subject.0,
                });
            }
            if r.teacher.idx() >= self.teachers.len() {
                errors.push(IntegrityError::RequirementTeacherOutOfRange {
                    requirement: r.id,
                    teacher: r.teacher.0,
                });
            }
            if let Some(room) = r.room {
                if room.idx() >= self.rooms.len() {
                    errors.push(IntegrityError::RequirementRoomOutOfRange {
                        requirement: r.id,
                        room: room.0,
                    });
                }
            }
        }
        errors
    }
}
