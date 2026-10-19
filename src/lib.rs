//! Native macOS driver for the HP LaserJet 1018 / 1020 / 1020 Plus / 1022 (ZjStream).

pub mod jbig;
pub mod raster;
pub mod zjs;

/// Local time as `YYYYmmddHHMMSS`, the format the PJL job attribute uses.
pub fn timestamp() -> String {
    // SAFETY: plain libc time calls on a zeroed, caller-owned `tm`.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}
