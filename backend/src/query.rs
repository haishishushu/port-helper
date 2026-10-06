//! 端口查询 / 进程反查的结果组装（纯逻辑，不直接调用系统 API）。

use std::collections::BTreeMap;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{
    HiddenState, PidQueryResult, PortBindings, PortQueryResult, ProcessDetail, ProcessGroup, Protocol,
    SocketBinding, SocketState,
};
use crate::process::ProcessCatalog;

/// 进程信息来源，便于在单测中替换。
pub trait ProcessSource {
    fn exists(&self, pid: u32) -> bool;
    fn detail(&self, pid: u32, port_hint: Option<u16>) -> ProcessDetail;
}

impl ProcessSource for ProcessCatalog {
    fn exists(&self, pid: u32) -> bool {
        ProcessCatalog::exists(self, pid)
    }
    fn detail(&self, pid: u32, port_hint: Option<u16>) -> ProcessDetail {
        ProcessCatalog::detail(self, pid, port_hint)
    }
}

pub fn validate_port(port: u32) -> AppResult<u16> {
    match u16::try_from(port) {
        Ok(p) if p >= 1 => Ok(p),
        _ => Err(AppError::new(ErrorCode::InvalidPort, "端口号需为 1–65535 之间的整数")),
    }
}

/// 默认只显示 LISTEN；UDP 没有状态，总是显示。
pub fn split_visible(bindings: Vec<SocketBinding>, include_all: bool) -> (Vec<SocketBinding>, Vec<HiddenState>) {
    if include_all {
        return (bindings, Vec::new());
    }
    let mut hidden: BTreeMap<SocketState, u32> = BTreeMap::new();
    let mut visible = Vec::new();
    for b in bindings {
        match b.state {
            Some(state) if state != SocketState::Listen => *hidden.entry(state).or_default() += 1,
            _ => visible.push(b),
        }
    }
    let hidden = hidden.into_iter().map(|(state, count)| HiddenState { state, count }).collect();
    (visible, hidden)
}

fn sort_bindings(bindings: &mut [SocketBinding]) {
    bindings.sort_by(|a, b| {
        (a.protocol != Protocol::Tcp, a.family as u8, a.state != Some(SocketState::Listen), a.local_port, &a.local_address)
            .cmp(&(b.protocol != Protocol::Tcp, b.family as u8, b.state != Some(SocketState::Listen), b.local_port, &b.local_address))
    });
}

pub fn port_result(port: u16, all: Vec<SocketBinding>, include_all: bool, src: &impl ProcessSource) -> PortQueryResult {
    let matched: Vec<SocketBinding> = all.into_iter().filter(|b| b.local_port == port).collect();
    let (visible, hidden) = split_visible(matched, include_all);

    let mut by_pid: BTreeMap<u32, Vec<SocketBinding>> = BTreeMap::new();
    for b in visible {
        by_pid.entry(b.pid).or_default().push(b);
    }
    let mut groups: Vec<ProcessGroup> = by_pid
        .into_iter()
        .map(|(pid, mut bindings)| {
            sort_bindings(&mut bindings);
            ProcessGroup { process: src.detail(pid, Some(port)), bindings }
        })
        .collect();
    // PID 0（内核持有的连接记录）放最后，其余按 PID 升序
    groups.sort_by_key(|g| (g.process.summary.pid == 0, g.process.summary.pid));
    PortQueryResult { port, groups, hidden, elapsed_ms: 0 }
}

pub fn pid_result(pid: u32, all: Vec<SocketBinding>, include_all: bool, src: &impl ProcessSource) -> AppResult<PidQueryResult> {
    if !src.exists(pid) {
        return Err(AppError::not_found(pid));
    }
    let owned: Vec<SocketBinding> = all.into_iter().filter(|b| b.pid == pid).collect();
    let (visible, hidden) = split_visible(owned, include_all);

    let mut tcp: BTreeMap<u16, Vec<SocketBinding>> = BTreeMap::new();
    let mut udp: BTreeMap<u16, Vec<SocketBinding>> = BTreeMap::new();
    for b in visible {
        let map = if b.protocol == Protocol::Tcp { &mut tcp } else { &mut udp };
        map.entry(b.local_port).or_default().push(b);
    }
    let collect = |m: BTreeMap<u16, Vec<SocketBinding>>| -> Vec<PortBindings> {
        m.into_iter()
            .map(|(port, mut bindings)| {
                sort_bindings(&mut bindings);
                PortBindings { port, bindings }
            })
            .collect()
    };
    Ok(PidQueryResult { process: src.detail(pid, None), tcp: collect(tcp), udp: collect(udp), hidden, elapsed_ms: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AddressFamily, AppIcon, OccupantKind, ProcessSummary};

    struct Fake;
    impl ProcessSource for Fake {
        fn exists(&self, pid: u32) -> bool {
            pid != 404
        }
        fn detail(&self, pid: u32, _: Option<u16>) -> ProcessDetail {
            let summary = ProcessSummary {
                pid,
                name: "node.exe".into(),
                app_name: "Node.js".into(),
                app_icon: AppIcon::Web,
                exe_path: None,
                start_time: Some(1),
                kind: OccupantKind::Normal,
                services: vec![],
                actionable: true,
                blocked_reason: None,
            };
            ProcessDetail { summary, command_line: None, cwd: None, parent_chain: vec![] }
        }
    }

    fn tcp(port: u16, state: SocketState, pid: u32, family: AddressFamily) -> SocketBinding {
        SocketBinding {
            protocol: Protocol::Tcp,
            family,
            local_address: if family == AddressFamily::Ipv4 { "0.0.0.0".into() } else { "::".into() },
            local_port: port,
            remote_address: None,
            remote_port: None,
            state: Some(state),
            pid,
        }
    }

    fn udp(port: u16, pid: u32) -> SocketBinding {
        SocketBinding { protocol: Protocol::Udp, state: None, ..tcp(port, SocketState::Listen, pid, AddressFamily::Ipv4) }
    }

    #[test]
    fn validates_port_range() {
        assert!(validate_port(0).is_err());
        assert!(validate_port(70_000).is_err());
        assert_eq!(validate_port(5173).unwrap(), 5173);
    }

    #[test]
    fn groups_by_pid_and_hides_non_listen() {
        let all = vec![
            tcp(5173, SocketState::Listen, 10, AddressFamily::Ipv6),
            tcp(5173, SocketState::Listen, 10, AddressFamily::Ipv4),
            tcp(5173, SocketState::Established, 10, AddressFamily::Ipv6),
            tcp(5173, SocketState::TimeWait, 0, AddressFamily::Ipv4),
            tcp(8080, SocketState::Listen, 20, AddressFamily::Ipv4),
            udp(5173, 30),
        ];
        let r = port_result(5173, all.clone(), false, &Fake);
        assert_eq!(r.groups.len(), 2);
        assert_eq!(r.groups[0].process.summary.pid, 10);
        assert_eq!(r.groups[0].bindings.len(), 2);
        assert_eq!(r.groups[0].bindings[0].family, AddressFamily::Ipv4);
        assert_eq!(r.groups[1].process.summary.pid, 30);
        assert_eq!(
            r.hidden,
            vec![
                HiddenState { state: SocketState::Established, count: 1 },
                HiddenState { state: SocketState::TimeWait, count: 1 }
            ]
        );

        let all_states = port_result(5173, all, true, &Fake);
        assert!(all_states.hidden.is_empty());
        assert_eq!(all_states.groups.last().unwrap().process.summary.pid, 0);
    }

    #[test]
    fn pid_result_splits_protocols() {
        let all = vec![
            tcp(8080, SocketState::Listen, 7, AddressFamily::Ipv4),
            tcp(8080, SocketState::Listen, 7, AddressFamily::Ipv6),
            tcp(5005, SocketState::Listen, 7, AddressFamily::Ipv4),
            udp(5353, 7),
            tcp(3000, SocketState::Listen, 8, AddressFamily::Ipv4),
        ];
        let r = pid_result(7, all, false, &Fake).unwrap();
        assert_eq!(r.tcp.iter().map(|p| p.port).collect::<Vec<_>>(), vec![5005, 8080]);
        assert_eq!(r.tcp[1].bindings.len(), 2);
        assert_eq!(r.udp.len(), 1);
        assert_eq!(pid_result(404, vec![], false, &Fake).unwrap_err().code, ErrorCode::ProcessNotFound);
    }
}
