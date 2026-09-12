//! 确定性测试数据生成器：20 个班、60 名教师、12 门科目、26 间教室。
//!
//! 生成一份合法输入（[`Problem`]）和一张满足全部硬约束的完整课表（[`Timetable`]），
//! 供单元测试与基准测试使用。同一种子必然生成同一结果（不依赖系统时间 / 随机源）。
//!
//! 规模：每周 5 天 × 8 节 = 40 槽；每班 35 节 / 周，全表共 700 节课。

use crate::bitmap::Bitmap;
use crate::model::*;

const N_CLASSES: usize = 20;
const TEACHERS_PER_SUBJECT: usize = 5;
const N_DAYS: u8 = 5;
const PERIODS_PER_DAY: u8 = 8;
const LUNCH_AFTER: u8 = 4;

const SUBJECT_NAMES: [&str; 12] = [
    "语文", "数学", "英语", "物理", "化学", "生物", "历史", "地理", "政治", "体育", "物理实验",
    "化学实验",
];

/// 每班每周开课计划：(科目下标, 周节数, 是否连堂)。合计 35 节。
const WEEKLY_PLAN: [(usize, u8, bool); 12] = [
    (0, 5, false),  // 语文
    (1, 5, false),  // 数学
    (2, 5, false),  // 英语
    (3, 3, false),  // 物理
    (4, 3, false),  // 化学
    (5, 2, false),  // 生物
    (6, 2, false),  // 历史
    (7, 2, false),  // 地理
    (8, 2, false),  // 政治
    (9, 2, false),  // 体育
    (10, 2, true),  // 物理实验：两节连上
    (11, 2, true),  // 化学实验：两节连上
];

const SUBJ_PE: usize = 9;
const SUBJ_PHYS_LAB: usize = 10;
const SUBJ_CHEM_LAB: usize = 11;

/// xorshift64* 伪随机数发生器：只为夹具服务，避免引入 rand 依赖。
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // 避免全零状态卡死。
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// [0, n) 均匀取值。
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

/// 生成合法输入与一张合法完整课表。`seed` 相同则结果完全相同。
///
/// 排课用带重试的贪心：先排连堂后排单节，任一需求排不下就换种子重来
/// （确定性：重试种子为 seed+1, seed+2, …）。
pub fn generate(seed: u64) -> (Problem, Timetable) {
    for attempt in seed..seed.wrapping_add(10_000) {
        if let Some(out) = try_generate(attempt) {
            return out;
        }
    }
    panic!("连续 10000 次尝试均未能排出合法课表（种子 {seed}）");
}

fn try_generate(seed: u64) -> Option<(Problem, Timetable)> {
    let mut rng = Rng::new(seed);
    let schedule = WeekSchedule::new(N_DAYS, PERIODS_PER_DAY, LUNCH_AFTER);
    let nslots = schedule.slot_count();

    // 科目。
    let subjects: Vec<Subject> = SUBJECT_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| Subject {
            id: SubjectId(i as u32),
            name: name.to_string(),
        })
        .collect();
    let nsubjects = subjects.len();

    // 教师：每科 5 名，共 60 名；每人随机 0..=5 个不可用槽。
    let mut teachers = Vec::new();
    for i in 0..nsubjects * TEACHERS_PER_SUBJECT {
        let mut unavailable = Bitmap::new(nslots);
        for _ in 0..rng.below(6) {
            unavailable.set(rng.below(nslots));
        }
        teachers.push(Teacher {
            id: TeacherId(i as u32),
            name: format!("教师{:02}", i + 1),
            unavailable,
        });
    }

    // 教室：20 间本班教室 + 物理/化学实验室各 2 间 + 操场 2 块。
    let mut rooms: Vec<Room> = (0..N_CLASSES)
        .map(|i| Room {
            id: RoomId(i as u32),
            name: format!("教室{}", i + 1),
        })
        .collect();
    for name in ["物理实验室1", "物理实验室2", "化学实验室1", "化学实验室2", "操场1", "操场2"] {
        rooms.push(Room {
            id: RoomId(rooms.len() as u32),
            name: name.to_string(),
        });
    }
    let lab_room = |subject: usize, class: usize| -> RoomId {
        // 同类专用教室两间，按班级奇偶分流。
        let base = match subject {
            SUBJ_PHYS_LAB => N_CLASSES,
            SUBJ_CHEM_LAB => N_CLASSES + 2,
            SUBJ_PE => N_CLASSES + 4,
            _ => unreachable!(),
        };
        RoomId((base + class % 2) as u32)
    };

    // 班级：前 10 个班高一，后 10 个班高二；本班教室与之一一对应。
    let classes: Vec<Class> = (0..N_CLASSES)
        .map(|i| {
            let (grade, num) = if i < 10 { ("高一", i + 1) } else { ("高二", i - 9) };
            Class {
                id: ClassId(i as u32),
                name: format!("{grade}({num})班"),
                home_room: RoomId(i as u32),
            }
        })
        .collect();

    // 需求：每班按计划各一条；授课教师在该科 5 名教师中挑当前负载最轻的。
    let mut requirements: Vec<Requirement> = Vec::new();
    let mut teacher_load = vec![0u32; teachers.len()];
    for (c, class) in classes.iter().enumerate() {
        for &(subject, lessons, double) in &WEEKLY_PLAN {
            let pool_start = subject * TEACHERS_PER_SUBJECT;
            let pool = &teacher_load[pool_start..pool_start + TEACHERS_PER_SUBJECT];
            let min_load = *pool.iter().min().unwrap();
            let candidates: Vec<usize> = pool
                .iter()
                .enumerate()
                .filter(|(_, &l)| l == min_load)
                .map(|(k, _)| pool_start + k)
                .collect();
            let teacher = candidates[rng.below(candidates.len())];
            teacher_load[teacher] += lessons as u32;

            let room = match subject {
                SUBJ_PHYS_LAB | SUBJ_CHEM_LAB | SUBJ_PE => Some(lab_room(subject, c)),
                _ => None,
            };
            requirements.push(Requirement {
                id: RequirementId(requirements.len() as u32),
                class: class.id,
                subject: SubjectId(subject as u32),
                teacher: TeacherId(teacher as u32),
                lessons_per_week: lessons,
                double_period: double,
                room,
            });
        }
    }

    // 排课：逐班进行，先连堂后单节；占用情况全部用位图维护。
    let mut class_busy = vec![Bitmap::new(nslots); N_CLASSES];
    let mut teacher_busy = vec![Bitmap::new(nslots); teachers.len()];
    let mut room_busy = vec![Bitmap::new(nslots); rooms.len()];
    // (班, 科目, 天) -> 已排节数，遵守「同班同科目每天不超过两节」。
    let mut daily = vec![0u8; N_CLASSES * nsubjects * N_DAYS as usize];
    let daily_idx = |class: usize, subject: usize, day: usize| {
        (class * nsubjects + subject) * N_DAYS as usize + day
    };
    let mut lessons: Vec<Lesson> = Vec::new();

    for (c, class) in classes.iter().enumerate() {
        let class_reqs: Vec<usize> = requirements
            .iter()
            .enumerate()
            .filter(|(_, r)| r.class == class.id)
            .map(|(i, _)| i)
            .collect();
        let mut doubles: Vec<usize> = class_reqs
            .iter()
            .copied()
            .filter(|&i| requirements[i].double_period)
            .collect();
        let mut singles: Vec<usize> = class_reqs
            .iter()
            .copied()
            .filter(|&i| !requirements[i].double_period)
            .collect();
        rng.shuffle(&mut doubles);
        rng.shuffle(&mut singles);

        // 连堂：候选为 (天, 起始节)，起始节须与下一节同处上午或下午。
        for ri in doubles {
            let req = &requirements[ri];
            let room = req.room_or_home(class).idx();
            let teacher = req.teacher.idx();
            let subject = req.subject.idx();
            for _ in 0..req.lessons_per_week / 2 {
                let mut starts: Vec<(u8, u8)> = (0..N_DAYS)
                    .flat_map(|d| [0u8, 1, 2, 4, 5, 6].into_iter().map(move |p| (d, p)))
                    .collect();
                rng.shuffle(&mut starts);
                let mut placed = false;
                for (day, period) in starts {
                    let s0 = schedule.slot_of(day, period).0 as usize;
                    let s1 = s0 + 1;
                    let fits = daily[daily_idx(c, subject, day as usize)] == 0
                        && !class_busy[c].get(s0)
                        && !class_busy[c].get(s1)
                        && !teacher_busy[teacher].get(s0)
                        && !teacher_busy[teacher].get(s1)
                        && !teachers[teacher].unavailable.get(s0)
                        && !teachers[teacher].unavailable.get(s1)
                        && !room_busy[room].get(s0)
                        && !room_busy[room].get(s1);
                    if fits {
                        class_busy[c].set(s0);
                        class_busy[c].set(s1);
                        teacher_busy[teacher].set(s0);
                        teacher_busy[teacher].set(s1);
                        room_busy[room].set(s0);
                        room_busy[room].set(s1);
                        daily[daily_idx(c, subject, day as usize)] += 2;
                        lessons.push(Lesson {
                            requirement: req.id,
                            slot: Slot(s0 as u32),
                            room: RoomId(room as u32),
                        });
                        lessons.push(Lesson {
                            requirement: req.id,
                            slot: Slot(s1 as u32),
                            room: RoomId(room as u32),
                        });
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    return None; // 换种子重试。
                }
            }
        }

        // 单节：逐节在全部槽位中找第一个可行的。
        for ri in singles {
            let req = &requirements[ri];
            let room = req.room_or_home(class).idx();
            let teacher = req.teacher.idx();
            let subject = req.subject.idx();
            for _ in 0..req.lessons_per_week {
                let mut order: Vec<usize> = (0..nslots).collect();
                rng.shuffle(&mut order);
                let mut placed = false;
                for s in order {
                    let day = schedule.day(Slot(s as u32)) as usize;
                    let fits = daily[daily_idx(c, subject, day)] < 2
                        && !class_busy[c].get(s)
                        && !teacher_busy[teacher].get(s)
                        && !teachers[teacher].unavailable.get(s)
                        && !room_busy[room].get(s);
                    if fits {
                        class_busy[c].set(s);
                        teacher_busy[teacher].set(s);
                        room_busy[room].set(s);
                        daily[daily_idx(c, subject, day)] += 1;
                        lessons.push(Lesson {
                            requirement: req.id,
                            slot: Slot(s as u32),
                            room: RoomId(room as u32),
                        });
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    return None;
                }
            }
        }
    }

    let problem = Problem {
        schedule,
        subjects,
        teachers,
        classes,
        rooms,
        requirements,
    };
    Some((problem, Timetable { lessons }))
}
