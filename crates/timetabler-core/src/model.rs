//! 排课问题的数据模型。
//!
//! 时间槽（[`Slot`]）是一周课表的线性编号：`slot = day * periods_per_day + period`。
//! 一周 5 天、每天 8 节时共 40 个槽，实体占用情况用等长位图表示（见 [`crate::bitmap`]）。

use crate::bitmap::Bitmap;

macro_rules! define_id {
    ($($name:ident),*) => {$(
        /// 强类型 id，避免不同实体的下标混用。值为在 `Problem` 对应 Vec 中的下标。
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub struct $name(pub u32);

        impl $name {
            #[inline]
            pub fn idx(self) -> usize {
                self.0 as usize
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "#{}", self.0)
            }
        }
    )*};
}

define_id!(TeacherId, ClassId, SubjectId, RoomId, RequirementId);

/// 时间槽：一周内的线性节次编号。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Slot(pub u32);

/// 节次表：每周天数、每天节数、午休位置（连堂不得跨越）。
#[derive(Clone, Debug)]
pub struct WeekSchedule {
    /// 每周上课天数，通常 5。
    pub days: u8,
    /// 每天节次数，通常 8。
    pub periods_per_day: u8,
    /// 上午节次数：节次 < 该值为上午，否则为下午。连堂的两节必须同在上午或同在下午。
    pub lunch_after_period: u8,
}

impl WeekSchedule {
    pub fn new(days: u8, periods_per_day: u8, lunch_after_period: u8) -> Self {
        assert!((1..=7).contains(&days), "每周天数应在 1..=7");
        assert!(periods_per_day >= 1, "每天至少 1 节");
        assert!(
            lunch_after_period <= periods_per_day,
            "午休位置不能超过每天节数"
        );
        WeekSchedule {
            days,
            periods_per_day,
            lunch_after_period,
        }
    }

    /// 一周总时间槽数。
    pub fn slot_count(&self) -> usize {
        self.days as usize * self.periods_per_day as usize
    }

    /// 由（天， 节次）构造时间槽。
    pub fn slot_of(&self, day: u8, period: u8) -> Slot {
        assert!(day < self.days && period < self.periods_per_day);
        Slot(day as u32 * self.periods_per_day as u32 + period as u32)
    }

    /// 时间槽所在的天（0 起）。
    pub fn day(&self, slot: Slot) -> u8 {
        (slot.0 / self.periods_per_day as u32) as u8
    }

    /// 时间槽在当天的节次（0 起）。
    pub fn period(&self, slot: Slot) -> u8 {
        (slot.0 % self.periods_per_day as u32) as u8
    }

    /// 两槽是否同一天。
    pub fn same_day(&self, a: Slot, b: Slot) -> bool {
        self.day(a) == self.day(b)
    }

    /// 两槽是否同一天且同在上午 / 下午（不跨中午）。
    pub fn same_block(&self, a: Slot, b: Slot) -> bool {
        self.same_day(a, b)
            && (self.period(a) < self.lunch_after_period)
                == (self.period(b) < self.lunch_after_period)
    }

    /// 两槽是否构成合法的连堂对：相邻节次、同一天、不跨中午。
    pub fn is_double_pair(&self, a: Slot, b: Slot) -> bool {
        self.same_block(a, b) && self.period(a).abs_diff(self.period(b)) == 1
    }

    /// 天的中文名（周一 … 周日）。
    pub fn day_name(&self, day: u8) -> &'static str {
        const NAMES: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
        NAMES.get(day as usize).copied().unwrap_or("周?")
    }

    /// 时间槽的人类可读描述，如「周三第5节」。
    pub fn describe_slot(&self, slot: Slot) -> String {
        format!(
            "{}第{}节",
            self.day_name(self.day(slot)),
            self.period(slot) + 1
        )
    }
}

#[derive(Clone, Debug)]
pub struct Subject {
    pub id: SubjectId,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Teacher {
    pub id: TeacherId,
    pub name: String,
    /// 不可用时段位图（长度 = 一周槽数）：置位表示该教师该槽不能排课。
    pub unavailable: Bitmap,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub id: ClassId,
    pub name: String,
    /// 本班教室：需求未指定教室时默认使用。
    pub home_room: RoomId,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
}

/// 周课时需求：某班某科目由某教师授课，每周若干节。
#[derive(Clone, Debug)]
pub struct Requirement {
    pub id: RequirementId,
    pub class: ClassId,
    pub subject: SubjectId,
    pub teacher: TeacherId,
    /// 每周课时数。
    pub lessons_per_week: u8,
    /// 是否连堂：为 true 时课节必须两两相邻成对，且不跨中午。
    pub double_period: bool,
    /// 指定教室（如实验室、操场）；None 表示用本班教室。
    ///
    /// Some 与 None 同为硬约束：课节必须排在 [`Requirement::room_or_home`]
    /// 决定的教室，否则校验器报 [`crate::ViolationKind::RoomMismatch`]。
    /// （当前按「指定到间」处理；「同类型教室任选」「普通课可浮动」的诉求
    /// 留待后续引入教室类型 / 浮动标记。）
    pub room: Option<RoomId>,
}

impl Requirement {
    /// 该需求实际使用的教室。
    pub fn room_or_home(&self, class: &Class) -> RoomId {
        self.room.unwrap_or(class.home_room)
    }
}

/// 排课问题的全部输入。
#[derive(Clone, Debug)]
pub struct Problem {
    pub schedule: WeekSchedule,
    pub subjects: Vec<Subject>,
    pub teachers: Vec<Teacher>,
    pub classes: Vec<Class>,
    pub rooms: Vec<Room>,
    pub requirements: Vec<Requirement>,
}

impl Problem {
    pub fn slot_count(&self) -> usize {
        self.schedule.slot_count()
    }

    pub fn requirement(&self, id: RequirementId) -> Option<&Requirement> {
        self.requirements.get(id.idx())
    }

    pub fn teacher_name(&self, id: TeacherId) -> &str {
        self.teachers.get(id.idx()).map_or("未知教师", |t| t.name.as_str())
    }

    pub fn class_name(&self, id: ClassId) -> &str {
        self.classes.get(id.idx()).map_or("未知班级", |c| c.name.as_str())
    }

    pub fn subject_name(&self, id: SubjectId) -> &str {
        self.subjects.get(id.idx()).map_or("未知科目", |s| s.name.as_str())
    }

    pub fn room_name(&self, id: RoomId) -> &str {
        self.rooms.get(id.idx()).map_or("未知教室", |r| r.name.as_str())
    }

    /// 需求的人类可读描述，如「高一(1)班的物理（教师05授课）」。
    pub fn requirement_desc(&self, id: RequirementId) -> String {
        match self.requirement(id) {
            Some(r) => format!(
                "{}的{}（{}授课）",
                self.class_name(r.class),
                self.subject_name(r.subject),
                self.teacher_name(r.teacher)
            ),
            None => format!("需求{}", id),
        }
    }
}

/// 一节课：某需求的一个课节被排到了某个时间槽、某个教室。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lesson {
    pub requirement: RequirementId,
    pub slot: Slot,
    pub room: RoomId,
}

/// 一张完整课表。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timetable {
    pub lessons: Vec<Lesson>,
}

impl Timetable {
    pub fn lesson_count(&self) -> usize {
        self.lessons.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_roundtrip() {
        let s = WeekSchedule::new(5, 8, 4);
        assert_eq!(s.slot_count(), 40);
        let slot = s.slot_of(2, 5);
        assert_eq!(slot.0, 21);
        assert_eq!(s.day(slot), 2);
        assert_eq!(s.period(slot), 5);
        assert_eq!(s.describe_slot(slot), "周三第6节");
    }

    #[test]
    fn double_pair_rules() {
        let s = WeekSchedule::new(5, 8, 4);
        // 上午相邻：合法
        assert!(s.is_double_pair(s.slot_of(0, 2), s.slot_of(0, 3)));
        // 第4节(上午末)与第5节(下午初)相邻但跨中午：不合法
        assert!(!s.is_double_pair(s.slot_of(0, 3), s.slot_of(0, 4)));
        // 隔天：不合法
        assert!(!s.is_double_pair(s.slot_of(0, 7), s.slot_of(1, 0)));
        // 同节次：不合法
        assert!(!s.is_double_pair(s.slot_of(1, 5), s.slot_of(1, 5)));
    }
}
