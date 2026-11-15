use std::time::{Duration, Instant};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

pub(super) struct ResourceMonitor {
    sampled_at: Instant,
    resident_bytes: Option<u64>,
    ui_millis: Option<f64>,
    ui_time_sum: Duration,
    ui_frames: u32,
    selected_raw_bytes: Option<u128>,
}

impl ResourceMonitor {
    pub(super) fn new() -> Self {
        Self {
            sampled_at: Instant::now() - SAMPLE_INTERVAL,
            resident_bytes: None,
            ui_millis: None,
            ui_time_sum: Duration::ZERO,
            ui_frames: 0,
            selected_raw_bytes: None,
        }
    }

    pub(super) fn sample(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.sampled_at) < SAMPLE_INTERVAL {
            return;
        }
        self.sampled_at = now;
        self.resident_bytes = resident_memory_bytes();
        if self.ui_frames > 0 {
            self.ui_millis = Some(self.ui_time_sum.as_secs_f64() * 1000.0 / self.ui_frames as f64);
        }
        self.ui_time_sum = Duration::ZERO;
        self.ui_frames = 0;
    }

    pub(super) fn record_ui_time(&mut self, elapsed: Duration) {
        self.ui_time_sum += elapsed;
        self.ui_frames += 1;
    }

    pub(super) fn next_sample_at(&self) -> Instant {
        self.sampled_at + SAMPLE_INTERVAL
    }

    pub(super) fn set_selected_raw_bytes(&mut self, bytes: Option<u128>) {
        self.selected_raw_bytes = bytes;
    }

    pub(super) fn label(&self, include_raw: bool) -> String {
        let ram = self
            .resident_bytes
            .map(|bytes| format_bytes(bytes as u128))
            .unwrap_or_else(|| "—".into());
        let ui = self
            .ui_millis
            .map(|ms| format!("{ms:.1} ms"))
            .unwrap_or_else(|| "—".into());
        let mut label = format!("RAM {ram}  ·  UI {ui}");
        if include_raw {
            if let Some(bytes) = self.selected_raw_bytes {
                label.push_str(&format!("  ·  Raw {}", format_bytes(bytes)));
            }
        }
        label
    }
}

fn format_bytes(bytes: u128) -> String {
    let value = bytes as f64;
    if bytes >= 1 << 30 {
        format!("{:.1} GiB", value / (1_u128 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.0} MiB", value / (1_u128 << 20) as f64)
    } else if bytes >= 1 << 10 {
        format!("{:.0} KiB", value / (1_u128 << 10) as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(target_os = "linux")]
fn resident_memory_bytes() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()?
                .checked_mul(1024)
        })
}

#[cfg(target_os = "macos")]
fn resident_memory_bytes() -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_taskinfo>();
    // SAFETY: The buffer has the size and layout requested by PROC_PIDTASKINFO.
    let written = unsafe {
        libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size as i32,
        )
    };
    (written == size as i32).then(|| unsafe { info.assume_init().pti_resident_size })
}

#[cfg(target_os = "windows")]
fn resident_memory_bytes() -> Option<u64> {
    use std::ffi::c_void;
    #[repr(C)]
    struct ProcessMemoryCounters {
        size: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        peak_paged_pool: usize,
        paged_pool: usize,
        peak_nonpaged_pool: usize,
        nonpaged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut counters = ProcessMemoryCounters {
        size: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_faults: 0,
        peak_working_set: 0,
        working_set: 0,
        peak_paged_pool: 0,
        paged_pool: 0,
        peak_nonpaged_pool: 0,
        nonpaged_pool: 0,
        pagefile: 0,
        peak_pagefile: 0,
    };
    // SAFETY: The buffer is initialized with the required size and layout.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.size) };
    (ok != 0).then_some(counters.working_set as u64)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn resident_memory_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_process_reports_resident_memory() {
        assert!(super::resident_memory_bytes().is_some_and(|bytes| bytes > 0));
    }
}
