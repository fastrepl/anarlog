const PROC_PIDT_SHORTBSDINFO: i32 = 13;
const MAXCOMLEN: usize = 16;

#[repr(C)]
struct ProcBsdShortInfo {
    pbsi_pid: u32,
    pbsi_ppid: u32,
    pbsi_pgid: u32,
    pbsi_status: u32,
    pbsi_comm: [u8; MAXCOMLEN],
    pbsi_flags: u32,
    pbsi_uid: u32,
    pbsi_gid: u32,
    pbsi_ruid: u32,
    pbsi_rgid: u32,
    pbsi_svuid: u32,
    pbsi_svgid: u32,
    pbsi_rfu: u32,
}

unsafe extern "C" {
    fn proc_pidinfo(
        pid: i32,
        flavor: i32,
        arg: u64,
        buffer: *mut std::ffi::c_void,
        buffersize: i32,
    ) -> i32;
}

pub fn parent_pid_for_pid(pid: u32) -> Option<u32> {
    let mut info = std::mem::MaybeUninit::<ProcBsdShortInfo>::uninit();
    let ret = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDT_SHORTBSDINFO,
            0,
            info.as_mut_ptr() as *mut std::ffi::c_void,
            std::mem::size_of::<ProcBsdShortInfo>() as i32,
        )
    };

    if ret >= std::mem::size_of::<ProcBsdShortInfo>() as i32 {
        Some(unsafe { info.assume_init() }.pbsi_ppid)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_pid_for_pid() {
        let pid = std::process::id();
        let parent_pid = parent_pid_for_pid(pid);
        assert!(parent_pid.is_some());
    }
}
