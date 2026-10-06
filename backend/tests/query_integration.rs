//! 集成测试：自己绑定随机端口，再通过真实的系统端口表查询，断言 PID 为当前进程。

use std::net::{TcpListener, UdpSocket};

use port_helper_lib::model::{AddressFamily, Protocol, SocketState};
use port_helper_lib::process::ProcessCatalog;
use port_helper_lib::{net, query};

#[test]
fn finds_own_tcp_ipv4_and_ipv6_listeners() {
    let v4 = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = v4.local_addr().unwrap().port();
    // 同一端口再绑 IPv6 回环（与 Vite 默认监听 ::1 的情况一致）
    let v6 = TcpListener::bind(("::1", port)).ok();

    let all = net::list_all().unwrap();
    let catalog = ProcessCatalog::load(&[std::process::id()]);
    let result = query::port_result(port, all, false, &catalog);

    assert_eq!(result.groups.len(), 1, "应只有当前进程占用该端口");
    let group = &result.groups[0];
    assert_eq!(group.process.summary.pid, std::process::id());
    assert!(group.process.summary.start_time.is_some());
    assert!(group.bindings.iter().all(|b| b.state == Some(SocketState::Listen)));
    assert!(group.bindings.iter().any(|b| b.family == AddressFamily::Ipv4 && b.local_address == "127.0.0.1"));
    if v6.is_some() {
        assert!(group.bindings.iter().any(|b| b.family == AddressFamily::Ipv6 && b.local_address == "::1"));
    }
}

#[test]
fn finds_own_udp_socket_and_reverse_lookup() {
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = udp.local_addr().unwrap().port();
    let pid = std::process::id();

    let all = net::list_all().unwrap();
    let catalog = ProcessCatalog::load(&[pid]);
    let result = query::pid_result(pid, all, false, &catalog).unwrap();

    let entry = result.udp.iter().find(|p| p.port == port).expect("反查结果应包含该 UDP 端口");
    assert_eq!(entry.bindings[0].protocol, Protocol::Udp);
    assert_eq!(result.process.summary.pid, pid);
    assert!(!result.process.parent_chain.is_empty());
}

#[test]
fn unused_port_returns_empty_groups() {
    // 先绑定再释放，拿到一个当前大概率空闲的端口
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let all = net::list_all().unwrap();
    let catalog = ProcessCatalog::load(&[]);
    let result = query::port_result(port, all, false, &catalog);
    assert!(result.groups.iter().all(|g| g.process.summary.pid != std::process::id()));
}
