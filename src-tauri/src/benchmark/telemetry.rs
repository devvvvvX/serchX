use rusqlite::Connection;

/// Returns peak resident set size (RSS) in bytes for the current process.
pub fn get_peak_rss_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            let mut rusage = std::mem::zeroed::<libc::rusage>();
            if libc::getrusage(libc::RUSAGE_SELF, &mut rusage) == 0 {
                // On macOS, ru_maxrss is reported in bytes
                rusage.ru_maxrss as u64
            } else {
                0
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        unsafe {
            let mut rusage = std::mem::zeroed::<libc::rusage>();
            if libc::getrusage(libc::RUSAGE_SELF, &mut rusage) == 0 {
                // On Linux, ru_maxrss is reported in kilobytes
                (rusage.ru_maxrss as u64) * 1024
            } else {
                0
            }
        }
    }
}
/// Returns current resident set size (RSS) in bytes for the current process.
pub fn get_current_rss_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    {
        #[allow(deprecated)]
        unsafe {
            let mut info = std::mem::zeroed::<libc::mach_task_basic_info>();
            let mut count = (std::mem::size_of::<libc::mach_task_basic_info>()
                / std::mem::size_of::<libc::natural_t>()) as libc::mach_msg_type_number_t;
            let kerr = libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut libc::mach_task_basic_info as libc::task_info_t,
                &mut count,
            );
            if kerr == libc::KERN_SUCCESS {
                info.resident_size
            } else {
                get_peak_rss_bytes()
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        get_peak_rss_bytes()
    }
}
/// Returns the current SQLite storage footprint in bytes based on page count * page size.
pub fn get_sqlite_disk_bytes(conn: &Connection) -> u64 {
    let page_count: i64 = conn
        .query_row("PRAGMA page_count;", [], |r| r.get(0))
        .unwrap_or(0);
    let page_size: i64 = conn
        .query_row("PRAGMA page_size;", [], |r| r.get(0))
        .unwrap_or(4096);

    (page_count * page_size) as u64
}
