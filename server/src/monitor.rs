use serde_json::{json, Value};
use std::path::Path;

#[cfg(windows)]
pub fn snapshot(data: &Path) -> Value {
    use parking_lot::Mutex;
    use std::{os::windows::ffi::OsStrExt, sync::OnceLock, time::Instant};
    use windows_sys::Win32::{
        Foundation::FILETIME,
        Storage::FileSystem::GetDiskFreeSpaceExW,
        System::{
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Threading::GetSystemTimes,
        },
    };
    type CpuSample = (u64, u64, Instant, Option<f64>);
    static PREVIOUS: OnceLock<Mutex<Option<CpuSample>>> = OnceLock::new();
    let mut previous = PREVIOUS.get_or_init(|| Mutex::new(None)).lock();
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // All pointers refer to initialized, writable structures of the Windows ABI.
    let times_ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
    let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
    let now = Instant::now();
    let cpu = if times_ok {
        let (total, idle) = (ticks(kernel) + ticks(user), ticks(idle));
        if let Some((pt, pi, at, last)) = *previous {
            if at.elapsed().as_secs_f64() < 1.0 {
                last
            } else {
                let dt = total.saturating_sub(pt);
                let pct =
                    (dt > 0).then(|| (100.0 * (1.0 - idle.saturating_sub(pi) as f64 / dt as f64)).clamp(0.0, 100.0));
                *previous = Some((total, idle, now, pct));
                pct
            }
        } else {
            *previous = Some((total, idle, now, None));
            None
        }
    } else {
        None
    };
    let mut mem = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    let memory_ok = unsafe { GlobalMemoryStatusEx(&mut mem) } != 0;
    let path: Vec<u16> = data.as_os_str().encode_wide().chain(Some(0)).collect();
    let (mut free, mut total, mut disk_free) = (0, 0, 0);
    let disk_ok = unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut free, &mut total, &mut disk_free) } != 0;
    json!({
        "cpu": cpu,
        "memoryUsed": memory_ok.then(|| mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys)),
        "memoryTotal": memory_ok.then_some(mem.ullTotalPhys),
        "diskFree": disk_ok.then_some(free), "diskTotal": disk_ok.then_some(total),
        "sampledAt": crate::util::now_ms(), "supported": true
    })
}

#[cfg(not(windows))]
pub fn snapshot(_data: &Path) -> Value {
    json!({"supported": false, "sampledAt": crate::util::now_ms(), "cpu": null,
        "memoryUsed": null, "memoryTotal": null, "diskFree": null, "diskTotal": null})
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn windows_memory_and_disk_are_real() {
        let dir = tempfile::tempdir().unwrap();
        let sample = super::snapshot(dir.path());
        assert!(sample["memoryTotal"].as_u64().unwrap() > 0);
        assert!(sample["memoryUsed"].as_u64().unwrap() <= sample["memoryTotal"].as_u64().unwrap());
        assert!(sample["diskTotal"].as_u64().unwrap() > 0);
    }
}
