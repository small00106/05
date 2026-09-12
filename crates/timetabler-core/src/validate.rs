//! 硬约束校验器：给定一张完整课表，报出所有违反的硬约束。
//!
//! 校验规则：
//! 1. 同一教师 / 班级 / 教室在同一时间槽只能有一节课（位图占用检测）。
//! 2. 教师的不可用时段必须避开。
//! 3. 连堂需求的课节必须两两相邻成对，且不跨中午。
//! 4. 每个班每天同一科目不超过两节。
//! 5. 每个需求的实际排课节数必须等于周课时需求（完整性）。
//!
//! 每条违反记录都说明：谁、在哪个时间槽、违反了哪条约束，
//! 可用 [`Violation::describe`] 生成中文描述。

use crate::bitmap::Bitmap;
use crate::model::*;

/// 违反的硬约束类别，附带相关实体。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViolationKind {
    /// 同一教师在同一时间槽被排了多节课。
    TeacherConflict { teacher: TeacherId },
    /// 同一班级在同一时间槽被排了多节课。
    ClassConflict { class: ClassId },
    /// 同一教室在同一时间槽被排了多节课。
    RoomConflict { room: RoomId },
    /// 课节落在了教师的不可用时段。
    TeacherUnavailable { teacher: TeacherId },
    /// 连堂需求的某个课节找不到相邻的配对课节（或配对跨了中午）。
    BrokenDoublePeriod { requirement: RequirementId },
    /// 某班某天同一科目超过两节。
    SubjectDailyLimit {
        class: ClassId,
        subject: SubjectId,
        count: u32,
    },
    /// 需求的实际排课节数与周课时不符（少排或多排）。
    RequirementCoverage {
        requirement: RequirementId,
        expected: u32,
        actual: u32,
    },
    /// 课表数据本身非法：时间槽越界。
    SlotOutOfRange { slot: u32 },
    /// 课表数据本身非法：引用了不存在的需求。
    UnknownRequirement { id: u32 },
    /// 课表数据本身非法：引用了不存在的教室。
    UnknownRoom { id: u32 },
}

/// 一条约束违反记录。`slot` 为 None 表示该违反与具体槽位无关（如课时不符）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub kind: ViolationKind,
    pub slot: Option<Slot>,
}

impl Violation {
    /// 生成中文描述，说明谁、在哪个时间槽、违反了哪条约束。
    pub fn describe(&self, p: &Problem) -> String {
        let at = self
            .slot
            .map(|s| p.schedule.describe_slot(s))
            .unwrap_or_default();
        match &self.kind {
            ViolationKind::TeacherConflict { teacher } => format!(
                "教师冲突：{} 在{}被同时安排了多节课",
                p.teacher_name(*teacher),
                at
            ),
            ViolationKind::ClassConflict { class } => format!(
                "班级冲突：{} 在{}有多节课同时进行",
                p.class_name(*class),
                at
            ),
            ViolationKind::RoomConflict { room } => format!(
                "教室冲突：{} 在{}被多节课同时使用",
                p.room_name(*room),
                at
            ),
            ViolationKind::TeacherUnavailable { teacher } => format!(
                "教师不可用：{} 在{}有课，但该时段已被标记为不可用",
                p.teacher_name(*teacher),
                at
            ),
            ViolationKind::BrokenDoublePeriod { requirement } => format!(
                "连堂未成对：{} 在{}的课节没有与之相邻成对的同需求课节（连堂须相邻且不跨中午）",
                p.requirement_desc(*requirement),
                at
            ),
            ViolationKind::SubjectDailyLimit {
                class,
                subject,
                count,
            } => format!(
                "科目日超限：{} 的 {} 在{}当天共排了 {} 节，超过 2 节上限",
                p.class_name(*class),
                p.subject_name(*subject),
                at,
                count
            ),
            ViolationKind::RequirementCoverage {
                requirement,
                expected,
                actual,
            } => format!(
                "课时不符：{} 应排 {} 节，实际排了 {} 节",
                p.requirement_desc(*requirement),
                expected,
                actual
            ),
            ViolationKind::SlotOutOfRange { slot } => format!(
                "数据非法：时间槽 {} 超出每周总槽数 {}",
                slot,
                p.slot_count()
            ),
            ViolationKind::UnknownRequirement { id } => {
                format!("数据非法：课表引用了不存在的需求 #{id}")
            }
            ViolationKind::UnknownRoom { id } => {
                format!("数据非法：课表引用了不存在的教室 #{id}")
            }
        }
    }
}

/// 每个班每天同一科目的节数上限。
const DAILY_SUBJECT_LIMIT: u32 = 2;

/// 对一张完整课表做全量硬约束校验，返回所有违反记录。
///
/// 实现要点：每个教师 / 班级 / 教室一个占用位图，逐课节置位检测冲突；
/// 同班同科目的日计数用扁平数组（不用 `HashMap`）。时间复杂度 O(课节数)。
pub fn validate(problem: &Problem, tt: &Timetable) -> Vec<Violation> {
    let nslots = problem.slot_count();
    let ndays = problem.schedule.days as usize;
    let nsubjects = problem.subjects.len();
    let mut violations = Vec::new();

    // 占用位图与「该槽冲突已报告」位图（同一槽的多节课只报一次）。
    let mut teacher_busy = vec![Bitmap::new(nslots); problem.teachers.len()];
    let mut teacher_reported = vec![Bitmap::new(nslots); problem.teachers.len()];
    let mut class_busy = vec![Bitmap::new(nslots); problem.classes.len()];
    let mut class_reported = vec![Bitmap::new(nslots); problem.classes.len()];
    let mut room_busy = vec![Bitmap::new(nslots); problem.rooms.len()];
    let mut room_reported = vec![Bitmap::new(nslots); problem.rooms.len()];

    // (班, 科目, 天) -> 节数；需求 -> 已排节数；需求 -> 各课节槽位（连堂检查用）。
    let mut daily_count = vec![0u32; problem.classes.len() * nsubjects * ndays];
    let mut covered = vec![0u32; problem.requirements.len()];
    let mut req_slots: Vec<Vec<Slot>> = vec![Vec::new(); problem.requirements.len()];

    for lesson in &tt.lessons {
        let Some(req) = problem.requirement(lesson.requirement) else {
            violations.push(Violation {
                kind: ViolationKind::UnknownRequirement {
                    id: lesson.requirement.0,
                },
                slot: None,
            });
            continue;
        };
        let s = lesson.slot.0 as usize;
        if s >= nslots {
            violations.push(Violation {
                kind: ViolationKind::SlotOutOfRange { slot: lesson.slot.0 },
                slot: None,
            });
            continue;
        }
        if lesson.room.idx() >= problem.rooms.len() {
            violations.push(Violation {
                kind: ViolationKind::UnknownRoom { id: lesson.room.0 },
                slot: Some(lesson.slot),
            });
            continue;
        }

        covered[req.id.idx()] += 1;
        req_slots[req.id.idx()].push(lesson.slot);

        // 1. 教师 / 班级 / 教室槽位冲突。
        let t = req.teacher.idx();
        if teacher_busy[t].set(s) && !teacher_reported[t].set(s) {
            violations.push(Violation {
                kind: ViolationKind::TeacherConflict {
                    teacher: req.teacher,
                },
                slot: Some(lesson.slot),
            });
        }
        let c = req.class.idx();
        if class_busy[c].set(s) && !class_reported[c].set(s) {
            violations.push(Violation {
                kind: ViolationKind::ClassConflict { class: req.class },
                slot: Some(lesson.slot),
            });
        }
        let r = lesson.room.idx();
        if room_busy[r].set(s) && !room_reported[r].set(s) {
            violations.push(Violation {
                kind: ViolationKind::RoomConflict { room: lesson.room },
                slot: Some(lesson.slot),
            });
        }

        // 2. 教师不可用时段。
        if problem.teachers[t].unavailable.get(s) {
            violations.push(Violation {
                kind: ViolationKind::TeacherUnavailable {
                    teacher: req.teacher,
                },
                slot: Some(lesson.slot),
            });
        }

        // 4. 同班同科目日计数（上限在循环后统一判定）。
        let day = problem.schedule.day(lesson.slot) as usize;
        daily_count[(c * nsubjects + req.subject.idx()) * ndays + day] += 1;
    }

    // 4. 每班每天同科目超过两节。
    for class in 0..problem.classes.len() {
        for subject in 0..nsubjects {
            for day in 0..ndays {
                let count = daily_count[(class * nsubjects + subject) * ndays + day];
                if count > DAILY_SUBJECT_LIMIT {
                    violations.push(Violation {
                        kind: ViolationKind::SubjectDailyLimit {
                            class: ClassId(class as u32),
                            subject: SubjectId(subject as u32),
                            count,
                        },
                        // 用当天第一节标记「哪一天」。
                        slot: Some(problem.schedule.slot_of(day as u8, 0)),
                    });
                }
            }
        }
    }

    // 3. 连堂成对检查：同需求的课节按天分组，升序贪心配对。
    for req in &problem.requirements {
        if !req.double_period {
            continue;
        }
        let mut by_day: Vec<Vec<Slot>> = vec![Vec::new(); ndays];
        for &slot in &req_slots[req.id.idx()] {
            by_day[problem.schedule.day(slot) as usize].push(slot);
        }
        for day_slots in &mut by_day {
            day_slots.sort_unstable();
            let mut i = 0;
            while i < day_slots.len() {
                let paired = i + 1 < day_slots.len()
                    && problem.schedule.is_double_pair(day_slots[i], day_slots[i + 1]);
                if paired {
                    i += 2;
                } else {
                    violations.push(Violation {
                        kind: ViolationKind::BrokenDoublePeriod { requirement: req.id },
                        slot: Some(day_slots[i]),
                    });
                    i += 1;
                }
            }
        }
    }

    // 5. 需求覆盖：实际节数必须等于周课时。
    for req in &problem.requirements {
        let actual = covered[req.id.idx()];
        let expected = req.lessons_per_week as u32;
        if actual != expected {
            violations.push(Violation {
                kind: ViolationKind::RequirementCoverage {
                    requirement: req.id,
                    expected,
                    actual,
                },
                slot: None,
            });
        }
    }

    violations
}
