//! PID → 该进程托管的 Windows 服务名。

use std::collections::HashMap;

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_MORE_DATA;
use windows::Win32::System::Services::{
    CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, ENUM_SERVICE_STATUS_PROCESSW,
    SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_ACTIVE, SERVICE_WIN32,
};

/// 读取正在运行的服务；失败时返回空表（服务名只是辅助信息，不影响查询）。
pub fn service_map() -> HashMap<u32, Vec<String>> {
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    unsafe {
        let Ok(scm) = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE) else {
            return map;
        };
        let mut needed = 0u32;
        let mut returned = 0u32;
        let mut resume = 0u32;
        // u64 缓冲保证结构体中指针字段的对齐
        let mut buf: Vec<u64> = vec![0; 64 * 1024 / 8];
        loop {
            let bytes = std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), buf.len() * 8);
            let result = EnumServicesStatusExW(
                scm,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_ACTIVE,
                Some(bytes),
                &mut needed,
                &mut returned,
                Some(&mut resume),
                PCWSTR::null(),
            );
            let entries = std::slice::from_raw_parts(
                buf.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
                returned as usize,
            );
            for e in entries {
                let pid = e.ServiceStatusProcess.dwProcessId;
                if pid != 0 {
                    if let Ok(name) = e.lpServiceName.to_string() {
                        map.entry(pid).or_default().push(name);
                    }
                }
            }
            match result {
                Ok(()) => break,
                Err(err) if err.code() == ERROR_MORE_DATA.to_hresult() => {
                    if needed as usize > buf.len() * 8 {
                        buf = vec![0; needed as usize / 8 + 1];
                    }
                }
                Err(_) => break,
            }
        }
        let _ = CloseServiceHandle(scm);
    }
    for names in map.values_mut() {
        names.sort();
    }
    map
}
