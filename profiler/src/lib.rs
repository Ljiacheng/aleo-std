// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the aleo-std library.

// The aleo-std library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The aleo-std library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the aleo-std library. If not, see <https://www.gnu.org/licenses/>.

#![allow(unused_imports)]
extern crate core;

pub use inner::*;

#[cfg(feature = "profiler")]
#[macro_use]
pub mod inner {
    pub use std::{sync::atomic::AtomicUsize, time::Instant};
    pub use std::collections::HashMap;
    pub use std::sync::RwLock;
    pub use std::time::Duration;
    pub use lazy_static::lazy_static;

    pub use colored::Colorize;

    lazy_static! {
        pub static ref STATISTIC: Statistic = Statistic::new();
    }

    pub static NUM_INDENT: AtomicUsize = AtomicUsize::new(0);
    pub const PAD_CHAR: &str = "·";

    pub struct Statistic {
        pub starts: RwLock<HashMap<String, Instant>>,
        pub ends: RwLock<HashMap<String, Instant>>,
        pub parts: RwLock<HashMap<String, (HashMap<String, Duration>, Duration)>>,
    }

    impl Statistic {
        pub fn new() -> Self {
            let statistic = Statistic {
                starts: RwLock::new(HashMap::new()),
                ends: RwLock::new(HashMap::new()),
                parts: RwLock::new(HashMap::new()),
            };
            statistic.start_work("DefaultWork");
            statistic
        }

        pub fn start_work(&self, msg: &str) {
            let mut starts_lock = self.starts.write().unwrap();
            match starts_lock.get(msg) {
                Some(_) => panic!("Duplicate start msg: {}", msg),
                None => { starts_lock.insert(msg.into(), std::time::Instant::now()); },
            }
        }

        pub fn end_work(&self, msg: &str) {
            let start = match self.starts.read().unwrap().get(msg) {
                Some(start) => start.clone(),
                None => panic!("{} Not Started", msg),
            };
            let now = std::time::Instant::now();
            if now < start {
                panic!("System Time Err: smaller end time than start, start: {:?}, end: {:?}", start, now);
            }
            self.ends.write().unwrap().insert(msg.into(), now);
        }

        pub fn end_job(&self, msg: &str, part: &str, work: &str, time: Duration) {
            if self.starts.read().unwrap().get(work).is_none() {
                panic!("Work {} Not started", work);
            }
            if self.ends.read().unwrap().get(work).is_some() {
                return;
            }
            let mut parts_lock = self.parts.write().unwrap();
            match parts_lock.get_mut(part) {
                Some((parts_map, part_time)) => {
                    match parts_map.get_mut(msg) {
                        Some(work_time) => *work_time += time,
                        None => { parts_map.insert(msg.into(), time); },
                    }
                    *part_time += time;
                },
                None => {
                    let mut parts_map = HashMap::new();
                    parts_map.insert(msg.into(), time);
                    parts_lock.insert(part.into(), (parts_map, time));
                }
            }
        }

        pub fn part_time(&self, part: &str) -> Duration {
            self.parts.read().unwrap().get(part)
                .map(|(_, t)| t.clone()).unwrap_or(Duration::default())
        }

        pub fn job_time(&self, msg: &str, part: &str) -> Duration {
            self.parts.read().unwrap().get(part)
                .map(|(parts_map, _)|
                    parts_map.get(msg)
                        .map(|t| t.clone()).unwrap_or(Duration::default())
                )
                .unwrap_or(Duration::default())
        }

        pub fn part_percent(&self, part: &str, work: &str) -> f64 {
            let work_total = self.work_total_time(work);
            let part_total = self.part_time(part);
            part_total.as_nanos() as f64 * 100.0 / work_total.as_nanos() as f64
        }

        pub fn job_percent(&self, msg: &str, part: &str, work: &str) -> f64 {
            let work_total = self.work_total_time(work);
            let job_total = self.job_time(msg, part);
            job_total.as_nanos() as f64 * 100.0 / work_total.as_nanos() as f64
        }

        pub fn work_total_time(&self, work: &str) -> Duration {
            match self.starts.read().unwrap().get(work) {
                Some(start) => match self.ends.read().unwrap().get(work) {
                    Some(end) => end.duration_since(*start),
                    None => {
                        if work == "DefaultWork" {
                            std::time::Instant::now().duration_since(*start)
                        } else {
                            panic!("Work {} Not ended", work)
                        }
                    },
                }
                None => panic!("Work {} Not started", work),
            }
        }
    }

    pub fn debug_duration(time: Duration) -> String {
        let secs = time.as_secs();
        let millis = time.subsec_millis();
        let micros = time.subsec_micros() % 1000;
        let nanos = time.subsec_nanos() % 1000;
        if secs != 0 {
            format!("{}.{:0>3}s", secs, millis)
        } else if millis > 0 {
            format!("{}.{:0>3}ms", millis, micros)
        } else if micros > 0 {
            format!("{}.{:0>3}µs", micros, nanos)
        } else {
            format!("{}ns", time.subsec_nanos())
        }
    }

    #[derive(Clone)]
    pub struct TimerInfo {
        pub msg: String,
        pub time: Instant,
    }

    #[macro_export]
    macro_rules! start_work_timer {
        ($work:expr) => {{
            pub use $crate::STATISTIC;
            STATISTIC.start_work(&$work());
        }}
    }

    #[macro_export]
    macro_rules! end_work_timer {
        ($work:expr) => {{
            pub use $crate::STATISTIC;
            STATISTIC.end_work(&$work());
        }}
    }

    #[macro_export]
    macro_rules! start_timer {
        ($msg:expr) => {{
            use std::{sync::atomic::Ordering, time::Instant};
            pub use $crate::*;

            let msg = $msg();
            let start_info = "Start:".yellow().bold();
            let indent_amount = 2 * NUM_INDENT.fetch_add(0, Ordering::Relaxed);
            let indent = compute_indent(indent_amount);

            println!("{}{:8} {}", indent, start_info, msg);
            NUM_INDENT.fetch_add(1, Ordering::Relaxed);

            $crate::TimerInfo {
                msg: msg.to_string(),
                time: Instant::now(),
            }
        }};
    }

    #[macro_export]
    macro_rules! end_timer {
        ($time:expr) => {{
            end_timer!($time, || "DefaultPart", || "DefaultWork");
        }};
        ($time:expr, $part:expr) => {{
            end_timer!($time, $part, || "DefaultWork");
        }};
        ($time:expr, $part:expr, $work:expr) => {{
            use std::sync::atomic::Ordering;
            pub use $crate::*;

            let time = $time.time;
            let part = $part();
            let final_time = time.elapsed();
            STATISTIC.end_job(&$time.msg, &part, &$work(), final_time.clone());

            let final_time = {
                let secs = final_time.as_secs();
                let millis = final_time.subsec_millis();
                let micros = final_time.subsec_micros() % 1000;
                let nanos = final_time.subsec_nanos() % 1000;
                if secs != 0 {
                    format!("{}.{:0>3}s", secs, millis).bold()
                } else if millis > 0 {
                    format!("{}.{:0>3}ms", millis, micros).bold()
                } else if micros > 0 {
                    format!("{}.{:0>3}µs", micros, nanos).bold()
                } else {
                    format!("{}ns", final_time.subsec_nanos()).bold()
                }
            };

            let end_info = "End:".green().bold();
            let part = if &part == &"DefaultPart" {
                ""
            } else {
                &part
            };
            let message = format!("{} {}", $time.msg, part);

            NUM_INDENT.fetch_sub(1, Ordering::Relaxed);
            let indent_amount = 2 * NUM_INDENT.fetch_add(0, Ordering::Relaxed);
            let indent = compute_indent(indent_amount);

            // Todo: Recursively ensure that *entire* string is of appropriate
            // width (not just message).
            println!(
                "{}{:8} {:.<pad$}{}",
                indent,
                end_info,
                message,
                final_time,
                pad = 75 - indent_amount
            );
        }};
    }

    #[macro_export]
    macro_rules! part_percent {
        ($part:expr) => {{
            time_percent!($part, || "DefaultWork");
        }};
        ($part:expr, $work:expr) => {{
            pub use $crate::*;
            let part = $part();
            let work = $work();
            let percent = STATISTIC.part_percent(&part, &work);
            println!("{}->{}: {:.3}% {}", work, part, percent, debug_duration(STATISTIC.part_time(&part)));
        }};
    }

    #[macro_export]
    macro_rules! job_percent {
        ($msg:expr) => {{
            job_percent!($msg:expr, || "DefaultPart", || "DefaultWork");
        }};
        ($msg:expr, $part:expr) => {{
            job_percent!($msg:expr, $part, || "DefaultWork");
        }};
        ($msg:expr, $part:expr, $work:expr) => {{
            pub use $crate::*;
            let msg = $msg();
            let part = $part();
            let work = $work();
            let percent = STATISTIC.job_percent(&msg, &part, &work);
            println!("{}->{}->{}: {:.3}% {}", work, part, msg, percent, debug_duration(STATISTIC.job_time(&msg, &part)));
        }};
    }

    #[macro_export]
    macro_rules! add_to_trace {
        ($title:expr, $msg:expr) => {{
            use std::sync::atomic::Ordering;
            pub use $crate::*;

            let start_msg = "StartMsg".yellow().bold();
            let end_msg = "EndMsg".green().bold();
            let title = $title();
            let start_msg = format!("{}: {}", start_msg, title);
            let end_msg = format!("{}: {}", end_msg, title);

            let start_indent_amount = 2 * NUM_INDENT.fetch_add(0, Ordering::Relaxed);
            let start_indent = compute_indent(start_indent_amount);

            let msg_indent_amount = 2 * NUM_INDENT.fetch_add(0, Ordering::Relaxed) + 2;
            let msg_indent = compute_indent_whitespace(msg_indent_amount);
            let mut final_message = "\n".to_string();
            for line in $msg().lines() {
                final_message += &format!("{}{}\n", msg_indent, line,);
            }

            // Todo: Recursively ensure that *entire* string is of appropriate
            // width (not just message).
            println!("{}{}", start_indent, start_msg);
            println!("{}{}", msg_indent, final_message,);
            println!("{}{}", start_indent, end_msg);
        }};
    }

    pub fn compute_indent_whitespace(indent_amount: usize) -> String {
        let mut indent = String::new();
        for _ in 0..indent_amount {
            indent.push(' ');
        }
        indent
    }

    pub fn compute_indent(indent_amount: usize) -> String {
        use std::env::var;
        let mut indent = String::new();
        let pad_string = match var("CLICOLOR") {
            Ok(val) => {
                if val == "0" {
                    " "
                } else {
                    PAD_CHAR
                }
            }
            Err(_) => PAD_CHAR,
        };
        for _ in 0..indent_amount {
            indent.push_str(&pad_string.white());
        }
        indent
    }
}

#[cfg(not(feature = "profiler"))]
#[macro_use]
mod inner {
    pub struct TimerInfo;

    #[macro_export]
    macro_rules! start_timer {
        ($msg:expr) => {
            $crate::TimerInfo
        };
    }
    #[macro_export]
    macro_rules! add_to_trace {
        ($title:expr, $msg:expr) => {
            let _ = $msg;
        };
    }

    #[macro_export]
    macro_rules! end_timer {
        ($time:expr, $msg:expr) => {
            let _ = $msg;
            let _ = $time;
        };
        ($time:expr) => {
            let _ = $time;
        };
    }
}

mod tests {
    use super::*;

    #[test]
    fn print_start_end() {
        start_work_timer!(|| "MyWork");

        let ot_start = start_timer!(|| "Hi_1");
        std::thread::sleep(std::time::Duration::from_millis(40));
        end_timer!(ot_start, || "Hi", || "MyWork");

        let start = start_timer!(|| "Hello_1");
        std::thread::sleep(std::time::Duration::from_millis(50));
        end_timer!(start, || "Hello", || "MyWork");

        let start = start_timer!(|| "Hello_1");
        std::thread::sleep(std::time::Duration::from_millis(50));
        end_timer!(start, || "Hello", || "MyWork");

        let start = start_timer!(|| "Hello_2");
        std::thread::sleep(std::time::Duration::from_millis(10));
        end_timer!(start, || "Hello", || "MyWork");

        end_work_timer!(|| "MyWork");

        part_percent!(|| "Hi", || "MyWork");
        part_percent!(|| "Hello", || "MyWork");
        job_percent!(|| "Hi_1", || "Hi", || "MyWork");
        job_percent!(|| "Hello_1", || "Hello", || "MyWork");
        job_percent!(|| "Hello_2", || "Hello", || "MyWork");
    }

    #[test]
    fn print_add() {
        let start = start_timer!(|| "Hello");
        add_to_trace!(|| "HelloMsg", || "Hello, I\nAm\nA\nMessage");
        end_timer!(start);
    }
}
