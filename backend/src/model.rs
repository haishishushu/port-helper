//! 与前端共享的数据结构。`cargo test` 时由 ts-rs 导出到 `frontend/src/bindings/`。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

/// TCP 连接状态（对应 MIB_TCP_STATE）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SocketState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
    Unknown,
}

impl SocketState {
    pub fn from_mib(value: u32) -> Self {
        match value {
            1 => Self::Closed,
            2 => Self::Listen,
            3 => Self::SynSent,
            4 => Self::SynReceived,
            5 => Self::Established,
            6 => Self::FinWait1,
            7 => Self::FinWait2,
            8 => Self::CloseWait,
            9 => Self::Closing,
            10 => Self::LastAck,
            11 => Self::TimeWait,
            12 => Self::DeleteTcb,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SocketBinding {
    pub protocol: Protocol,
    pub family: AddressFamily,
    pub local_address: String,
    pub local_port: u16,
    pub remote_address: Option<String>,
    pub remote_port: Option<u16>,
    /// UDP 没有状态
    pub state: Option<SocketState>,
    pub pid: u32,
}

/// 占用者类型，决定前端的提示与可执行操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OccupantKind {
    Normal,
    Service,
    Forwarder,
    Critical,
    System,
    Idle,
    #[serde(rename = "self")]
    #[ts(rename = "self")]
    SelfProcess,
}

/// 前端图标映射用的应用分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AppIcon {
    Generic,
    Web,
    Java,
    Python,
    Database,
    Server,
    Container,
    Terminal,
    System,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessSummary {
    pub pid: u32,
    pub name: String,
    pub app_name: String,
    pub app_icon: AppIcon,
    pub exe_path: Option<String>,
    /// 进程创建时间（Unix 毫秒），用于操作前校验 PID 未被复用
    pub start_time: Option<u64>,
    pub kind: OccupantKind,
    pub services: Vec<String>,
    pub actionable: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParentLink {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub summary: ProcessSummary,
    pub command_line: Option<String>,
    pub cwd: Option<String>,
    /// 从最上层祖先到当前进程（含当前进程）
    pub parent_chain: Vec<ParentLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessGroup {
    /// 直接携带详情（命令行、工作目录、父进程链），前端不再单独请求详情
    pub process: ProcessDetail,
    pub bindings: Vec<SocketBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HiddenState {
    pub state: SocketState,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortQueryResult {
    pub port: u16,
    pub groups: Vec<ProcessGroup>,
    pub hidden: Vec<HiddenState>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortBindings {
    pub port: u16,
    pub bindings: Vec<SocketBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PidQueryResult {
    pub process: ProcessDetail,
    pub tcp: Vec<PortBindings>,
    pub udp: Vec<PortBindings>,
    pub hidden: Vec<HiddenState>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CloseMethod {
    WindowClose,
    ConsoleCtrlC,
    /// macOS / Linux：发送 SIGTERM
    Signal,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CloseOutcome {
    Exited,
    Timeout,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CloseResult {
    pub outcome: CloseOutcome,
    pub method: CloseMethod,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KillResult {
    pub exited: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrivilegeInfo {
    pub elevated: bool,
}
