//! 进程信息读取：sysinfo 负责名称/路径/命令行/工作目录，Win32 负责父 PID 与精确的创建时间。
//!
//! 性能要点（prd/todo/perf-todo.md P1-4）：任何“全量枚举进程”在 Windows 上都要约 7ms，
//! 所以每次查询只允许枚举一次。父进程链不靠全量枚举，而是对目标进程逐级查询父 PID，
//! 再把“目标 + 祖先”一次性交给 sysinfo 刷新。

pub mod classify;
pub mod services;

use std::collections::{HashMap, HashSet};
use std::os::windows::ffi::OsStringExt;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows::core::PWSTR;
use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessBasicInformation};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_BASIC_INFORMATION, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::error::{is_invalid_parameter, AppError, AppResult};
use crate::model::{ParentLink, ProcessDetail, ProcessSummary};
use classify::{classify, ClassifyInput};

const MAX_PARENT_DEPTH: usize = 6;
/// 1601-01-01 到 1970-01-01 的 100ns 间隔数
const FILETIME_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

/// 一次查询内共享的进程快照，只包含目标进程及其祖先。
pub struct ProcessCatalog {
    sys: System,
    services: HashMap<u32, Vec<String>>,
    self_pid: u32,
}

impl ProcessCatalog {
    /// 读取目标进程及其祖先链（只做一次全量枚举）。
    pub fn load(targets: &[u32]) -> Self {
        let mut wanted: Vec<Pid> = Vec::new();
        let mut seen = HashSet::new();
        for &pid in targets {
            let mut current = pid;
            for _ in 0..=MAX_PARENT_DEPTH {
                if current == 0 || !seen.insert(current) {
                    break;
                }
                wanted.push(Pid::from_u32(current));
                match parent_pid(current) {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
        }
        let mut sys = System::new();
        if !wanted.is_empty() {
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&wanted),
                true,
                ProcessRefreshKind::nothing()
                    .with_exe(UpdateKind::OnlyIfNotSet)
                    .with_cmd(UpdateKind::OnlyIfNotSet)
                    .with_cwd(UpdateKind::OnlyIfNotSet),
            );
        }
        Self { sys, services: services::service_map(), self_pid: std::process::id() }
    }

    pub fn exists(&self, pid: u32) -> bool {
        pid == 0 || pid == 4 || self.sys.process(Pid::from_u32(pid)).is_some()
    }

    pub fn name(&self, pid: u32) -> String {
        match self.sys.process(Pid::from_u32(pid)) {
            Some(p) => p.name().to_string_lossy().into_owned(),
            None if pid == 0 => "System Idle Process".into(),
            None if pid == 4 => "System".into(),
            None => format!("PID {pid}"),
        }
    }

    fn exe_path(&self, pid: u32) -> Option<String> {
        let p = self.sys.process(Pid::from_u32(pid))?;
        p.exe().map(|e| e.to_string_lossy().into_owned()).filter(|s| !s.is_empty())
    }

    fn command_line(&self, pid: u32) -> Option<String> {
        let p = self.sys.process(Pid::from_u32(pid))?;
        let args: Vec<String> = p
            .cmd()
            .iter()
            .map(|a| {
                let s = a.to_string_lossy();
                if s.contains(' ') && !s.starts_with('"') { format!("\"{s}\"") } else { s.into_owned() }
            })
            .collect();
        (!args.is_empty()).then(|| args.join(" "))
    }

    fn cwd(&self, pid: u32) -> Option<String> {
        let p = self.sys.process(Pid::from_u32(pid))?;
        p.cwd().map(|c| c.to_string_lossy().into_owned()).filter(|s| !s.is_empty())
    }

    fn summary(&self, pid: u32, port_hint: Option<u16>, command_line: Option<&str>) -> ProcessSummary {
        let name = self.name(pid);
        let exe_path = self.exe_path(pid);
        let services = self.services.get(&pid).cloned().unwrap_or_default();
        let c = classify(&ClassifyInput {
            pid,
            name: &name,
            exe_path: exe_path.as_deref(),
            command_line,
            services: &services,
            port_hint,
            self_pid: self.self_pid,
        });
        let start_time = if pid == 0 || pid == 4 { None } else { start_time(pid).ok().flatten() };
        // 读不到启动时间就无法做 PID 复用校验，不允许操作
        let (actionable, blocked_reason) = if c.actionable && start_time.is_none() {
            (false, Some("无法读取进程启动时间，可能需要管理员权限".to_string()))
        } else {
            (c.actionable, c.blocked_reason)
        };
        ProcessSummary {
            pid,
            name,
            app_name: c.app_name,
            app_icon: c.app_icon,
            exe_path,
            start_time,
            kind: c.kind,
            services,
            actionable,
            blocked_reason,
        }
    }

    /// 进程详情；`port_hint` 用于区分 PID 4 是 HTTP.sys 还是 SMB。
    pub fn detail(&self, pid: u32, port_hint: Option<u16>) -> ProcessDetail {
        let command_line = self.command_line(pid);
        ProcessDetail {
            summary: self.summary(pid, port_hint, command_line.as_deref()),
            cwd: self.cwd(pid),
            parent_chain: self.parent_chain(pid),
            command_line,
        }
    }

    fn parent_chain(&self, pid: u32) -> Vec<ParentLink> {
        let mut chain = vec![ParentLink { pid, name: self.name(pid) }];
        let mut seen = HashSet::from([pid]);
        let mut child = self.sys.process(Pid::from_u32(pid));
        while chain.len() <= MAX_PARENT_DEPTH {
            let Some((parent_pid, child_start)) = child.and_then(|c| c.parent().map(|p| (p, c.start_time()))) else {
                break;
            };
            let Some(parent) = self.sys.process(parent_pid) else { break };
            // 父进程退出后 PID 可能被新进程复用：“父进程”比子进程启动得晚就说明不是真正的父进程
            if !seen.insert(parent_pid.as_u32()) || parent.start_time() > child_start {
                break;
            }
            chain.push(ParentLink { pid: parent_pid.as_u32(), name: self.name(parent_pid.as_u32()) });
            child = Some(parent);
        }
        chain.reverse();
        chain
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
fn parent_pid(pid: u32) -> Option<u32> {
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
