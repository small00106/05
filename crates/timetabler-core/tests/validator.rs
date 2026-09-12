//! 校验器集成测试：手工构造的小问题（精确断言每类违反）+ 20 班 60 教师大夹具。

use timetabler_core::bitmap::Bitmap;
use timetabler_core::testgen;
use timetabler_core::*;

// ---------------------------------------------------------------------------
// 手工小问题：2 天 × 4 节（第 2 节后午休），2 班 3 师 2 科 2 教室。
// ---------------------------------------------------------------------------

fn t_id(i: u32) -> TeacherId {
    TeacherId(i)
}

fn tiny_problem() -> Problem {
    let schedule = WeekSchedule::new(2, 4, 2);
    let nslots = schedule.slot_count();

    let subjects = ["语文", "数学"]
        .iter()
        .enumerate()
        .map(|(i, n)| Subject {
            id: SubjectId(i as u32),
            name: n.to_string(),
        })
        .collect();

    let mut unavail_t0 = Bitmap::new(nslots);
    unavail_t0.set(3); // 教师甲 周一下午第一节(槽3)不可用
    let teachers = vec![
        Teacher {
            id: t_id(0),
            name: "教师甲".into(),
            unavailable: unavail_t0,
        },
        Teacher {
            id: t_id(1),
            name: "教师乙".into(),
            unavailable: Bitmap::new(nslots),
        },
        Teacher {
            id: t_id(2),
            name: "教师丙".into(),
            unavailable: Bitmap::new(nslots),
        },
    ];

    let rooms = vec![
        Room {
            id: RoomId(0),
            name: "教室一".into(),
        },
        Room {
            id: RoomId(1),
            name: "教室二".into(),
        },
    ];

    let classes = vec![
        Class {
            id: ClassId(0),
            name: "甲班".into(),
            home_room: RoomId(0),
        },
        Class {
            id: ClassId(1),
            name: "乙班".into(),
            home_room: RoomId(1),
        },
    ];

    let mk_req = |id: u32, class: u32, subject: u32, teacher: u32, n: u8, double: bool| {
        Requirement {
            id: RequirementId(id),
            class: ClassId(class),
            subject: SubjectId(subject),
            teacher: t_id(teacher),
            lessons_per_week: n,
            double_period: double,
            room: None,
        }
    };
    let requirements = vec![
        mk_req(0, 0, 0, 0, 2, true),  // 甲班 语文 教师甲 连堂 2 节
        mk_req(1, 0, 1, 2, 1, false), // 甲班 数学 教师丙 1 节
        mk_req(2, 1, 0, 2, 2, false), // 乙班 语文 教师丙 2 节
        mk_req(3, 1, 1, 1, 1, false), // 乙班 数学 教师乙 1 节
        mk_req(4, 0, 0, 1, 1, false), // 甲班 语文 教师乙 1 节
    ];

    Problem {
        schedule,
        subjects,
        teachers,
        classes,
        rooms,
        requirements,
    }
}

fn lesson(req: u32, slot: u32, room: u32) -> Lesson {
    Lesson {
        requirement: RequirementId(req),
        slot: Slot(slot),
        room: RoomId(room),
    }
}

/// 一张合法课表（已逐条人工核对：无冲突、连堂成对、日限与课时均满足）。
fn valid_timetable() -> Timetable {
    Timetable {
        lessons: vec![
            lesson(0, 0, 0), // 甲班语文连堂：周一 1-2 节
            lesson(0, 1, 0),
            lesson(1, 4, 0), // 甲班数学：周二第1节
            lesson(2, 1, 1), // 乙班语文：周一第2节、周二第2节
            lesson(2, 5, 1),
            lesson(3, 6, 1), // 乙班数学：周二第3节
            lesson(4, 7, 0), // 甲班语文(乙)：周二第4节
        ],
    }
}

fn kinds_of(violations: &[Violation]) -> Vec<&ViolationKind> {
    violations.iter().map(|v| &v.kind).collect()
}

#[test]
fn valid_timetable_passes() {
    let p = tiny_problem();
    let tt = valid_timetable();
    assert_eq!(validate(&p, &tt), Vec::<Violation>::new());
}

#[test]
fn detects_teacher_conflict() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 甲班数学(教师丙)从槽4挪到槽5：教师丙在槽5已有乙班语文课。
    tt.lessons[2].slot = Slot(5);
    let v = validate(&p, &tt);
    assert_eq!(
        kinds_of(&v),
        vec![&ViolationKind::TeacherConflict {
            teacher: t_id(2)
        }]
    );
    assert_eq!(v[0].slot, Some(Slot(5)));
}

#[test]
fn detects_class_conflict() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 甲班数学挪到槽0：甲班槽0已有语文连堂（同教室，故同时报教室冲突）。
    tt.lessons[2].slot = Slot(0);
    let v = validate(&p, &tt);
    let kinds = kinds_of(&v);
    assert!(kinds.contains(&&ViolationKind::ClassConflict { class: ClassId(0) }));
}

#[test]
fn detects_room_conflict() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 甲班数学(教师丙, 槽4)挪到槽6并借用教室二：乙班数学(教师乙)正占用。
    tt.lessons[2].slot = Slot(6);
    tt.lessons[2].room = RoomId(1);
    let v = validate(&p, &tt);
    assert_eq!(
        kinds_of(&v),
        vec![&ViolationKind::RoomConflict { room: RoomId(1) }]
    );
}

#[test]
fn detects_teacher_unavailable() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 甲班语文连堂挪到周一 3-4 节(槽2,3)：教师甲槽3不可用。
    tt.lessons[0].slot = Slot(2);
    tt.lessons[1].slot = Slot(3);
    let v = validate(&p, &tt);
    assert_eq!(
        kinds_of(&v),
        vec![&ViolationKind::TeacherUnavailable {
            teacher: t_id(0)
        }]
    );
    assert_eq!(v[0].slot, Some(Slot(3)));
}

#[test]
fn detects_broken_double_period_not_adjacent() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 连堂拆成不相邻的槽0和槽2。
    tt.lessons[1].slot = Slot(2);
    let v = validate(&p, &tt);
    let broken: Vec<_> = v
        .iter()
        .filter(|x| matches!(x.kind, ViolationKind::BrokenDoublePeriod { .. }))
        .collect();
    assert_eq!(broken.len(), 2); // 两节各自找不到配对
    assert_eq!(
        broken[0].kind,
        ViolationKind::BrokenDoublePeriod {
            requirement: RequirementId(0)
        }
    );
}

#[test]
fn detects_broken_double_period_crossing_lunch() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 连堂排在槽1(上午末)和槽2(下午初)：相邻但跨中午。
    tt.lessons[0].slot = Slot(1);
    tt.lessons[1].slot = Slot(2);
    let v = validate(&p, &tt);
    let broken: Vec<_> = v
        .iter()
        .filter(|x| matches!(x.kind, ViolationKind::BrokenDoublePeriod { .. }))
        .collect();
    assert_eq!(broken.len(), 2);
}

#[test]
fn detects_subject_daily_limit() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 甲班语文(乙)从槽7挪到槽2：甲班周一语文变成 3 节。
    tt.lessons[6].slot = Slot(2);
    let v = validate(&p, &tt);
    assert_eq!(
        kinds_of(&v),
        vec![&ViolationKind::SubjectDailyLimit {
            class: ClassId(0),
            subject: SubjectId(0),
            count: 3,
        }]
    );
    assert_eq!(v[0].slot, Some(Slot(0))); // 用当天第一节标记是哪一天
}

#[test]
fn detects_requirement_coverage() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    // 删掉乙班数学唯一一节课。
    tt.lessons.remove(5);
    let v = validate(&p, &tt);
    assert_eq!(
        kinds_of(&v),
        vec![&ViolationKind::RequirementCoverage {
            requirement: RequirementId(3),
            expected: 1,
            actual: 0,
        }]
    );
}

#[test]
fn detects_slot_out_of_range() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    tt.lessons[5].slot = Slot(99);
    let v = validate(&p, &tt);
    let kinds = kinds_of(&v);
    assert!(kinds.contains(&&ViolationKind::SlotOutOfRange { slot: 99 }));
    // 越界的课节不计入课时覆盖。
    assert!(kinds.contains(&&ViolationKind::RequirementCoverage {
        requirement: RequirementId(3),
        expected: 1,
        actual: 0,
    }));
}

#[test]
fn violation_describe_mentions_who_when_what() {
    let p = tiny_problem();
    let mut tt = valid_timetable();
    tt.lessons[2].slot = Slot(5);
    let v = validate(&p, &tt);
    let msg = v[0].describe(&p);
    assert!(msg.contains("教师丙"), "应说明是谁：{msg}");
    assert!(msg.contains("周二第2节"), "应说明时间槽：{msg}");
    assert!(msg.contains("教师冲突"), "应说明违反了哪条约束：{msg}");
}

// ---------------------------------------------------------------------------
// 大夹具：20 个班、60 名教师。
// ---------------------------------------------------------------------------

#[test]
fn generated_fixture_is_valid_and_deterministic() {
    let (p, tt) = testgen::generate(42);
    assert_eq!(p.classes.len(), 20);
    assert_eq!(p.teachers.len(), 60);
    assert_eq!(tt.lesson_count(), 20 * 35);
    assert_eq!(validate(&p, &tt), Vec::<Violation>::new());

    // 同一种子生成结果完全一致。
    let (p2, tt2) = testgen::generate(42);
    assert_eq!(tt, tt2);
    assert_eq!(validate(&p2, &tt2), Vec::<Violation>::new());
}

#[test]
fn corrupted_fixture_is_caught() {
    let (p, tt) = testgen::generate(7);
    let mut bad = tt.clone();
    // 把某班一节课挪到本班另一节课的槽位：必然造成班级冲突。
    let first = tt.lessons[0];
    let req0 = p.requirement(first.requirement).unwrap();
    let same_class = tt
        .lessons
        .iter()
        .position(|l| {
            l.slot != first.slot
                && p.requirement(l.requirement).unwrap().class == req0.class
        })
        .expect("夹具中每班有多节课");
    bad.lessons[same_class].slot = first.slot;
    let v = validate(&p, &bad);
    assert!(
        v.iter().any(|x| x.kind
            == ViolationKind::ClassConflict {
                class: req0.class
            }),
        "应报出班级冲突：{v:?}"
    );
}
