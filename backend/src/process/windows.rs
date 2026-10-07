//! Windows：父 PID、映像名、精确创建时间直接走 Win32，只打开单个进程。

use std::os::windows::ffi::OsStringExt;

use windows::core::PWSTR;
use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessBasicInformation};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_BASIC_INFORMATION, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::error::{is_invalid_parameter, AppError, AppResult};

/// 1601-01-01 到 1970-01-01 的 100ns 间隔数
const FILETIME_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

/// 不对应真实进程的 PID：0 = System Idle Process，4 = System（内核）
pub fn pseudo_name(pid: u32) -> Option<&'static str> {
    match pid {
        0 => Some("System Idle Process"),
        4 => Some("System"),
        _ => None,
    }
}

fn open_limited(pid: u32) -> AppResult<Option<HANDLE>> {
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => Ok(Some(h)),
            Err(e) if is_invalid_parameter(&e) => Ok(None),
            Err(e) => Err(AppError::win32("OpenProcess", &e)),
        }
    }
}

/// 父进程 PID（只查询单个进程，不做全量枚举）；读不到时返回 None。
pub fn parent_pid(pid: u32) -> Option<u32> {
    if pid == 4 {
        return None;
    }
    let handle = open_limited(pid).ok().flatten()?;
    unsafe {
        let mut info = PROCESS_BASIC_INFORMATION::default();
        let status = NtQueryInformationProcess(
            handle,
            ProcessBasicInformation,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32,
            std::ptr::null_mut(),
        );
        let _ = CloseHandle(handle);
        let parent = info.InheritedFromUniqueProcessId as u32;
        (status.is_ok() && parent != 0).then_some(parent)
    }
}

/// 进程映像文件名（如 `lsass.exe`），只打开单个进程。进程不存在返回 `Ok(None)`。
pub fn image_name(pid: u32) -> AppResult<Option<String>> {
    let Some(handle) = open_limited(pid)? else { return Ok(None) };
    unsafe {
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        result.map_err(|e| AppError::win32("QueryFullProcessImageNameW", &e))?;
        let path = std::ffi::OsString::from_wide(&buf[..len as usize]);
        let path = std::path::PathBuf::from(path);
        Ok(path.file_name().map(|n| n.to_string_lossy().into_owned()))
    }
}

/// 进程创建时间（Unix 毫秒）。进程不存在或已退出返回 `Ok(None)`。
pub fn start_time(pid: u32) -> AppResult<Option<u64>> {
    let Some(handle) = open_limited(pid)? else { return Ok(None) };
    unsafe {
        let (mut creation, mut exit, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let result = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        result.map_err(|e| AppError::win32("GetProcessTimes", &e))?;
        // 进程已退出但仍有句柄未关闭时，内核对象还在、能被打开；有退出时间即视为不存在
        if exit.dwHighDateTime != 0 || exit.dwLowDateTime != 0 {
            return Ok(None);
        }
        let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
        Ok(Some(ticks.saturating_sub(FILETIME_UNIX_EPOCH) / 10_000))
    }
}
