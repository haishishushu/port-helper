//! Windows：通过 IpHelper 读取端口表。
//!
//! 表结构按 Windows 文档逐字节解析，不依赖具体的结构体绑定，便于单测：
//! - MIB_TCPTABLE_OWNER_PID：dwNumEntries + N × 6 个 u32
//! - MIB_TCP6TABLE_OWNER_PID：dwNumEntries + N × 56 字节
//! - MIB_UDPTABLE_OWNER_PID：dwNumEntries + N × 3 个 u32
//! - MIB_UDP6TABLE_OWNER_PID：dwNumEntries + N × 28 字节

use std::ffi::c_void;
use std::net::{Ipv4Addr, Ipv6Addr};

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

use crate::error::{AppError, AppResult};
use crate::model::{AddressFamily, Protocol, SocketBinding, SocketState};

const TCP4_ROW: usize = 24;
const TCP6_ROW: usize = 56;
const UDP4_ROW: usize = 12;
const UDP6_ROW: usize = 28;

/// 读取全部 TCP / UDP 记录。
pub fn list_all() -> AppResult<Vec<SocketBinding>> {
    let mut out = Vec::new();
    out.extend(parse_tcp4(&fetch_tcp(AF_INET.0 as u32)?));
    out.extend(parse_tcp6(&fetch_tcp(AF_INET6.0 as u32)?));
    out.extend(parse_udp4(&fetch_udp(AF_INET.0 as u32)?));
    out.extend(parse_udp6(&fetch_udp(AF_INET6.0 as u32)?));
    Ok(out)
}

fn fetch_tcp(family: u32) -> AppResult<Vec<u8>> {
    fetch_table("GetExtendedTcpTable", |buf, size| unsafe {
        GetExtendedTcpTable(buf, size, false, family, TCP_TABLE_OWNER_PID_ALL, 0)
    })
}

fn fetch_udp(family: u32) -> AppResult<Vec<u8>> {
    fetch_table("GetExtendedUdpTable", |buf, size| unsafe {
        GetExtendedUdpTable(buf, size, false, family, UDP_TABLE_OWNER_PID, 0)
    })
}

/// 先探测所需大小再读取；表在两次调用之间可能变大，所以最多重试几次。
fn fetch_table(api: &str, call: impl Fn(Option<*mut c_void>, *mut u32) -> u32) -> AppResult<Vec<u8>> {
    let mut size: u32 = 0;
    let code = call(None, &mut size);
    if code != ERROR_INSUFFICIENT_BUFFER.0 && code != NO_ERROR.0 {
        return Err(AppError::win32_code(api, code));
    }
    for _ in 0..4 {
        // 用 u64 缓冲保证对齐，再按字节视图返回
        let mut buf = vec![0u64; (size as usize).div_ceil(8) + 1];
        let mut cap = (buf.len() * 8) as u32;
        let code = call(Some(buf.as_mut_ptr().cast()), &mut cap);
        if code == NO_ERROR.0 {
            let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), cap as usize) };
            return Ok(bytes.to_vec());
        }
        if code != ERROR_INSUFFICIENT_BUFFER.0 {
            return Err(AppError::win32_code(api, code));
        }
        size = cap + 1024;
    }
    Err(AppError::win32_code(api, ERROR_INSUFFICIENT_BUFFER.0))
}

fn u32_at(buf: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap())
}

/// 端口字段为 DWORD，低 16 位是网络字节序的端口号。
fn port_at(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([buf[offset], buf[offset + 1]])
}

fn ipv4_at(buf: &[u8], offset: usize) -> String {
    let b: [u8; 4] = buf[offset..offset + 4].try_into().unwrap();
    Ipv4Addr::from(b).to_string()
}

fn ipv6_at(buf: &[u8], offset: usize) -> String {
    let b: [u8; 16] = buf[offset..offset + 16].try_into().unwrap();
    Ipv6Addr::from(b).to_string()
}

/// 返回每一行的起始偏移；表头只有 dwNumEntries 一个 u32。
fn rows(buf: &[u8], row_size: usize) -> impl Iterator<Item = usize> + '_ {
    let count = if buf.len() >= 4 { u32_at(buf, 0) as usize } else { 0 };
    let max = buf.len().saturating_sub(4) / row_size;
    (0..count.min(max)).map(move |i| 4 + i * row_size)
}

/// 监听状态没有有意义的远端地址。
fn remote(state: SocketState, addr: String, port: u16) -> (Option<String>, Option<u16>) {
    if state == SocketState::Listen {
        (None, None)
    } else {
        (Some(addr), Some(port))
    }
}

pub fn parse_tcp4(buf: &[u8]) -> Vec<SocketBinding> {
    rows(buf, TCP4_ROW)
        .map(|o| {
            let state = SocketState::from_mib(u32_at(buf, o));
            let (remote_address, remote_port) = remote(state, ipv4_at(buf, o + 12), port_at(buf, o + 16));
            SocketBinding {
                protocol: Protocol::Tcp,
                family: AddressFamily::Ipv4,
                local_address: ipv4_at(buf, o + 4),
                local_port: port_at(buf, o + 8),
                remote_address,
                remote_port,
                state: Some(state),
                pid: u32_at(buf, o + 20),
            }
        })
        .collect()
}

pub fn parse_tcp6(buf: &[u8]) -> Vec<SocketBinding> {
    rows(buf, TCP6_ROW)
        .map(|o| {
            // ucLocalAddr[16] | dwLocalScopeId | dwLocalPort | ucRemoteAddr[16] | dwRemoteScopeId | dwRemotePort | dwState | dwOwningPid
            let state = SocketState::from_mib(u32_at(buf, o + 48));
            let (remote_address, remote_port) = remote(state, ipv6_at(buf, o + 24), port_at(buf, o + 44));
            SocketBinding {
                protocol: Protocol::Tcp,
                family: AddressFamily::Ipv6,
                local_address: ipv6_at(buf, o),
                local_port: port_at(buf, o + 20),
                remote_address,
                remote_port,
                state: Some(state),
                pid: u32_at(buf, o + 52),
            }
        })
        .collect()
}

pub fn parse_udp4(buf: &[u8]) -> Vec<SocketBinding> {
    rows(buf, UDP4_ROW)
        .map(|o| SocketBinding {
            protocol: Protocol::Udp,
            family: AddressFamily::Ipv4,
            local_address: ipv4_at(buf, o),
            local_port: port_at(buf, o + 4),
            remote_address: None,
            remote_port: None,
            state: None,
            pid: u32_at(buf, o + 8),
        })
        .collect()
}

pub fn parse_udp6(buf: &[u8]) -> Vec<SocketBinding> {
    rows(buf, UDP6_ROW)
        .map(|o| SocketBinding {
            protocol: Protocol::Udp,
            family: AddressFamily::Ipv6,
            local_address: ipv6_at(buf, o),
            local_port: port_at(buf, o + 20),
            remote_address: None,
            remote_port: None,
            state: None,
            pid: u32_at(buf, o + 24),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port_field(port: u16) -> [u8; 4] {
        let p = port.to_be_bytes();
        [p[0], p[1], 0, 0]
    }

    #[test]
    fn parses_tcp4_row() {
        let mut buf = 1u32.to_ne_bytes().to_vec();
        buf.extend(2u32.to_ne_bytes()); // LISTEN
        buf.extend([127, 0, 0, 1]);
        buf.extend(port_field(5173));
        buf.extend([0, 0, 0, 0]);
        buf.extend(port_field(0));
        buf.extend(18244u32.to_ne_bytes());
        let rows = parse_tcp4(&buf);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].local_address, "127.0.0.1");
        assert_eq!(rows[0].local_port, 5173);
        assert_eq!(rows[0].state, Some(SocketState::Listen));
        assert_eq!(rows[0].remote_address, None);
        assert_eq!(rows[0].pid, 18244);
    }

    #[test]
    fn parses_udp6_row() {
        let mut buf = 1u32.to_ne_bytes().to_vec();
        let mut addr = [0u8; 16];
        addr[15] = 1; // ::1
        buf.extend(addr);
        buf.extend(0u32.to_ne_bytes());
        buf.extend(port_field(5353));
        buf.extend(42u32.to_ne_bytes());
        let rows = parse_udp6(&buf);
        assert_eq!(rows[0].local_address, "::1");
        assert_eq!(rows[0].local_port, 5353);
        assert_eq!(rows[0].state, None);
        assert_eq!(rows[0].pid, 42);
    }

    #[test]
    fn truncated_table_is_safe() {
        let mut buf = 5u32.to_ne_bytes().to_vec(); // 声称 5 行但只有半行
        buf.extend([0u8; 10]);
        assert!(parse_tcp4(&buf).is_empty());
        assert!(parse_tcp4(&[]).is_empty());
    }
}
