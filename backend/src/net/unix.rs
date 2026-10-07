//! macOS / Linux：通过 netstat2 读取端口表（macOS 走 libproc，Linux 走 netlink + /proc）。
//!
//! 非 root 运行时读不到其他用户进程的套接字归属，Linux 上 TIME_WAIT 等记录也没有所属进程，
//! 这些记录的 PID 记为 0，由分类逻辑给出说明。

use std::net::IpAddr;

use netstat2::{iterate_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{AddressFamily, Protocol, SocketBinding, SocketState};

/// 读取全部 TCP / UDP 记录；同一套接字被多个进程共享时，每个进程各占一行。
/// 单条记录读取失败（例如扫描过程中套接字刚好关闭）时跳过该条，不影响整体结果。
pub fn list_all() -> AppResult<Vec<SocketBinding>> {
    let sockets = iterate_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP,
    )
    .map_err(|e| AppError::new(ErrorCode::SystemApiFailed, format!("读取端口表失败：{e}")))?;

    let mut out = Vec::new();
    for info in sockets.flatten() {
        let base = match info.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp) => {
                let state = map_state(tcp.state);
                let listen = state == SocketState::Listen;
                SocketBinding {
                    protocol: Protocol::Tcp,
                    family: family(&tcp.local_addr),
                    local_address: tcp.local_addr.to_string(),
                    local_port: tcp.local_port,
                    remote_address: (!listen).then(|| tcp.remote_addr.to_string()),
                    remote_port: (!listen).then_some(tcp.remote_port),
                    state: Some(state),
                    pid: 0,
                }
            }
            ProtocolSocketInfo::Udp(udp) => SocketBinding {
                protocol: Protocol::Udp,
                family: family(&udp.local_addr),
                local_address: udp.local_addr.to_string(),
                local_port: udp.local_port,
                remote_address: None,
                remote_port: None,
                state: None,
                pid: 0,
            },
        };
        if info.associated_pids.is_empty() {
            out.push(base);
        } else {
            for pid in info.associated_pids {
                out.push(SocketBinding { pid, ..base.clone() });
            }
        }
    }
    Ok(out)
}

fn family(addr: &IpAddr) -> AddressFamily {
    if addr.is_ipv4() { AddressFamily::Ipv4 } else { AddressFamily::Ipv6 }
}

fn map_state(state: TcpState) -> SocketState {
    match state {
        TcpState::Closed => SocketState::Closed,
        TcpState::Listen => SocketState::Listen,
        TcpState::SynSent => SocketState::SynSent,
        TcpState::SynReceived => SocketState::SynReceived,
        TcpState::Established => SocketState::Established,
        TcpState::FinWait1 => SocketState::FinWait1,
        TcpState::FinWait2 => SocketState::FinWait2,
        TcpState::CloseWait => SocketState::CloseWait,
        TcpState::Closing => SocketState::Closing,
        TcpState::LastAck => SocketState::LastAck,
        TcpState::TimeWait => SocketState::TimeWait,
        TcpState::DeleteTcb => SocketState::DeleteTcb,
        #[allow(unreachable_patterns)]
        _ => SocketState::Unknown,
    }
}
