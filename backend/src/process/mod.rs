//! 进程信息读取：sysinfo 负责名称/路径/命令行/工作目录，父 PID、映像名与创建时间按平台实现
//! （Windows 走 Win32 拿到毫秒精度，macOS / Linux 走 sysinfo 单进程刷新）。
//!
//! 性能要点（prd/todo/perf-todo.md P1-4）：任何“全量枚举进程”在 Windows 上都要约 7ms，
//! 所以每次查询只允许枚举一次。父进程链不靠全量枚举，而是对目标进程逐级查询父 PID，
//! 再把“目标 + 祖先”一次性交给 sysinfo 刷新。

pub mod classify;
pub mod services;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::collections::{HashMap, HashSet};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::model::{ParentLink, ProcessDetail, ProcessSummary};
use classify::{classify, ClassifyInput};
#[cfg(unix)]
pub use self::unix::{image_name, pseudo_name, start_time};
#[cfg(unix)]
use self::unix::parent_pid;
#[cfg(windows)]
pub use self::windows::{image_name, pseudo_name, start_time};
#[cfg(windows)]
use self::windows::parent_pid;

const MAX_PARENT_DEPTH: usize = 6;

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
        let pids: Vec<u32> = wanted.iter().map(|p| p.as_u32()).collect();
        Self { sys, services: services::service_map(&pids), self_pid: std::process::id() }
    }

    pub fn exists(&self, pid: u32) -> bool {
        pseudo_name(pid).is_some() || self.sys.process(Pid::from_u32(pid)).is_some()
    }

    pub fn name(&self, pid: u32) -> String {
        match (self.sys.process(Pid::from_u32(pid)), pseudo_name(pid)) {
            (Some(p), _) => p.name().to_string_lossy().into_owned(),
            (None, Some(name)) => name.into(),
            (None, None) => format!("PID {pid}"),
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
        let start_time = if pseudo_name(pid).is_some() { None } else { start_time(pid).ok().flatten() };
        // 读不到启动时间就无法做 PID 复用校验，不允许操作
        let (actionable, blocked_reason) = if c.actionable && start_time.is_none() {
            (false, Some(format!("无法读取进程启动时间，可能需要{}权限", crate::privilege::ADMIN_TERM)))
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

    /// 进程详情；`port_hint` 用于区分 Windows 上 PID 4 是 HTTP.sys 还是 SMB。
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
