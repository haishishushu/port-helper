# port-helper 前后端实现方案

> 依据：`prd/todo/todo.md`（需求）、`prd/ui/port-helper.pen`（UI 设计）。
> 读者：参与 port-helper 开发的工程师。

---

## 1. 本期范围

| 范围 | 内容 |
| --- | --- |
| 本期实现（MVP + 已设计的 P1） | 端口查询（TCP/UDP、IPv4/IPv6、默认只看 LISTEN）、进程详情、应用识别、特殊占用者识别、正常关闭（降级策略）、强制结束、PID+启动时间校验、系统关键进程二次确认、Docker/WSL 转发提示、立即释放端口、进程反查、常用端口（最近查询优先）、设置（主题/LISTEN/关闭等待时长/清空常用端口）、以管理员身份重新启动、Toast、自绘标题栏 |
| 本期不做 | 结束进程树（P1，UI 已设计，接口预留）、全部端口列表、进程管理页、系统托盘、全局快捷键、SQLite、自动更新、macOS/Linux |

## 2. 目录结构

```text
port-helper/
├── package.json              # 根脚本：调用 Tauri CLI（dev / build）
├── backend/                  # Tauri 2 + Rust（原 src-tauri）
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/
│   ├── .cargo/config.toml    # ts-rs 导出目录
│   └── src/
│       ├── main.rs           # 入口；处理 --ctrl-c 辅助模式
│       ├── lib.rs            # Tauri Builder、插件、命令注册
│       ├── commands.rs       # #[tauri::command]，只做参数校验与线程调度
│       ├── error.rs          # AppError / ErrorCode
│       ├── model.rs          # 与前端共享的数据结构（ts-rs 导出）
│       ├── query.rs          # 端口查询 / 进程反查的组装逻辑（纯逻辑，可单测）
│       ├── net.rs            # GetExtendedTcpTable / GetExtendedUdpTable
│       ├── process/
│       │   ├── mod.rs        # 进程信息读取（sysinfo + Win32）
│       │   ├── classify.rs   # 应用识别、特殊占用者识别（纯逻辑，可单测）
│       │   └── services.rs   # PID → Windows 服务名
│       ├── ops.rs            # 正常关闭、强制结束、身份校验
│       ├── console.rs        # Ctrl+C 辅助进程
│       └── privilege.rs      # 是否管理员、以管理员身份重启
└── frontend/                 # React 19 + TS + Vite + Tailwind 4 + shadcn/ui
    ├── package.json
    ├── vite.config.ts
    ├── components.json       # shadcn 配置
    └── src/
        ├── bindings/         # ts-rs 生成的类型（不要手改）
        ├── lib/              # api、settings、recent-ports、format、utils
        ├── hooks/
        ├── components/
        │   ├── ui/           # shadcn 组件
        │   ├── layout/       # TitleBar、Sidebar
        │   ├── query/        # 查询框、结果卡片、详情面板、状态
        │   └── dialogs/      # 全部确认框
        └── pages/            # PortQueryPage、PidQueryPage、SettingsPage
```

## 3. 开发与构建

| 场景 | 命令（在仓库根目录） |
| --- | --- |
| 安装依赖 | `pnpm install`（根） + `pnpm --dir frontend install` |
| 开发运行 | `pnpm dev`（= `tauri dev`，自动启动 Vite） |
| 生产构建 | `pnpm build`（= `tauri build`，输出 NSIS 安装包） |
| 后端检查 | `cargo check --manifest-path backend/Cargo.toml` |
| 后端测试 + 导出 TS 类型 | `cargo test --manifest-path backend/Cargo.toml` |
| 前端类型检查 + 打包 | `pnpm --dir frontend build` |

`tauri.conf.json` 关键配置：`build.devUrl = http://localhost:5173`，`build.frontendDist = ../frontend/dist`，`beforeDevCommand / beforeBuildCommand` 在 `../frontend` 下执行。

## 4. 总体架构

```text
React 页面 ──► lib/api.ts ──invoke──► commands.rs ──spawn_blocking──► query / ops / privilege
   ▲                                                                     │
   │  bindings/*.ts（ts-rs 生成）                                          ▼
   └──────── 统一 { code, message } 错误 ◄──── error.rs ◄──── net / process / console（Win32）

设置与常用端口：前端直接使用 tauri-plugin-store（settings.json），后端不参与。
```

原则：

- 后端对外只暴露“业务级” Command，不暴露任意系统调用或 SQL。
- 所有耗时系统调用放在 `spawn_blocking`，不阻塞 Tauri 主线程。
- 单个进程信息读取失败只影响该字段（返回 `null`），不导致整个查询失败。
- 进程操作在后端做最终安全校验（保护进程、身份校验、关键进程确认），前端的确认框只是第一道防线。

## 5. 接口契约

字段统一 camelCase；时间统一为 Unix 毫秒（`number`）。

### 5.1 Commands

| Command | 入参 | 返回 | 说明 |
| --- | --- | --- | --- |
| `query_port` | `{ port: number, includeAllStates: boolean }` | `PortQueryResult` | 按本地端口查询 |
| `query_pid` | `{ pid: number, includeAllStates: boolean }` | `PidQueryResult` | 进程反查 |
| `close_process` | `{ pid, startTime, timeoutMs, confirmCritical }` | `CloseResult` | 正常关闭（含等待） |
| `kill_process` | `{ pid, startTime, confirmCritical }` | `KillResult` | 强制结束 |
| `get_privilege` | — | `PrivilegeInfo` | 是否管理员 |
| `relaunch_as_admin` | — | — | 成功后当前进程退出 |

### 5.2 主要类型

```ts
type Protocol = "tcp" | "udp";
type AddressFamily = "ipv4" | "ipv6";
type SocketState = "listen" | "established" | "timeWait" | "closeWait" | "synSent"
  | "synReceived" | "finWait1" | "finWait2" | "closing" | "lastAck" | "closed" | "deleteTcb" | "unknown";

interface SocketBinding {
  protocol: Protocol; family: AddressFamily;
  localAddress: string; localPort: number;
  remoteAddress: string | null; remotePort: number | null;
  state: SocketState | null;          // UDP 为 null
  pid: number;
}

type OccupantKind = "normal" | "service" | "forwarder" | "critical" | "system" | "idle" | "self";

interface ProcessSummary {
  pid: number; name: string;
  appName: string;                      // 识别结果，如 “Vite 开发服务器”
  appIcon: AppIcon;                     // 前端图标映射用
  exePath: string | null;
  startTime: number | null;             // 进程创建时间，用于身份校验
  kind: OccupantKind;
  services: string[];                   // 该进程托管的 Windows 服务
  actionable: boolean;                  // 是否允许关闭/结束
  blockedReason: string | null;         // 不可操作原因
}

interface ProcessDetail extends ProcessSummary {
  commandLine: string | null;
  cwd: string | null;
  parentChain: ParentLink[];            // 从最上层到当前进程
}

interface ProcessGroup { process: ProcessDetail; bindings: SocketBinding[] }  // 直接携带详情，前端不再单独请求
interface HiddenState { state: SocketState; count: number }

interface PortQueryResult { port: number; groups: ProcessGroup[]; hidden: HiddenState[]; elapsedMs: number }
interface PortBindings { port: number; bindings: SocketBinding[] }
interface PidQueryResult { process: ProcessDetail; tcp: PortBindings[]; udp: PortBindings[]; hidden: HiddenState[]; elapsedMs: number }

type CloseMethod = "windowClose" | "consoleCtrlC" | "none";
type CloseOutcome = "exited" | "timeout" | "unsupported";
interface CloseResult { outcome: CloseOutcome; method: CloseMethod; elapsedMs: number }
interface KillResult { exited: boolean; elapsedMs: number }
interface PrivilegeInfo { elevated: boolean }
interface AppError { code: ErrorCode; message: string }
```

### 5.3 错误码

| code | 场景 | 前端表现 |
| --- | --- | --- |
| `INVALID_PORT` / `INVALID_PID` | 参数越界 | 输入框下方红字 |
| `PROCESS_NOT_FOUND` | PID 不存在 / 已退出 | 反查：PID 不存在状态；操作：Toast“进程已退出”并重新查询 |
| `PROCESS_CHANGED` | PID 已被复用（启动时间不一致） | Toast“PID 已被其他进程复用，操作已取消” + 重新查询 |
| `PROTECTED_PROCESS` | PID 0 / 4 / 自身 | 按钮本就禁用；兜底 Toast |
| `CRITICAL_CONFIRM_REQUIRED` | 关键进程未二次确认 | 弹出关键进程确认框 |
| `ACCESS_DENIED` | 权限不足 | Toast + “以管理员身份重新启动” |
| `UAC_CANCELLED` | 用户取消 UAC | Toast“已取消提升权限” |
| `SYSTEM_API_FAILED` | Win32 调用失败 | 查询失败状态，附带 API 名和错误码 |
| `INTERNAL` | 其他 | Toast |

## 6. 后端设计

### 6.1 端口表读取（net.rs）

1. 对 `AF_INET`、`AF_INET6` 分别调用 `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)` 与 `GetExtendedUdpTable(UDP_TABLE_OWNER_PID)`。
2. 先传空缓冲区拿到所需大小，`ERROR_INSUFFICIENT_BUFFER` 时扩容重试（最多 3 次，应对表在两次调用间变大）。
3. 端口字段为网络字节序（低 16 位），IPv4 地址按内存字节序直接转为 `Ipv4Addr`。
4. 解析结果统一为 `SocketBinding`，查询端只做过滤，不关心来源。

### 6.2 查询组装（query.rs，纯逻辑）

- `query_port`：过滤 `localPort == port` → 按“显示全部状态”开关拆分可见/隐藏（UDP 无状态，总是可见）→ 按 PID 分组 → 为每个 PID 读取 `ProcessDetail`（含命令行、工作目录、父进程链）→ 排序（PID 0 放最后，其余按 PID 升序）。
- `query_pid`：先确认进程存在，否则 `PROCESS_NOT_FOUND`；过滤 `pid == 目标` → 按协议、端口分组。
- 同一进程同时监听 `0.0.0.0` 与 `::` 时保留为同组的两条绑定，前端合并展示。

### 6.3 进程信息（process/）

| 字段 | 来源 | 失败处理 |
| --- | --- | --- |
| 父 PID | `NtQueryInformationProcess(ProcessBasicInformation)` 逐级向上查询，不做全量枚举 | 停止向上 |
| 名称、EXE、命令行、工作目录 | `sysinfo` 一次刷新“目标 + 祖先”（每次查询只全量枚举一次，见 perf-todo P1-4） | 读不到为 `null` |
| 启动时间 | `OpenProcess(QUERY_LIMITED)` + `GetProcessTimes`，转 Unix 毫秒 | `null` → 不可操作 |
| 托管服务 | `EnumServicesStatusExW(SC_ENUM_PROCESS_INFO)`，一次查询建一张 PID→服务表 | 空数组 |
| 父进程链 | 沿 `parent` 向上最多 6 层，遇到已退出或循环即停 | 截断 |

**应用识别（classify.rs）**：先看进程名，再看命令行与路径关键词。

| 进程 | 规则（命令行/路径包含） | appName |
| --- | --- | --- |
| node.exe | vite / next / nuxt / webpack / react-scripts / @nestjs、nest start / nodemon | Vite 开发服务器 / Next.js / Nuxt / Webpack Dev Server / Create React App / NestJS / nodemon；默认 Node.js |
| java.exe、javaw.exe | spring-boot、org.springframework.boot / JetBrains、idea / catalina | Spring Boot / IntelliJ IDEA / Tomcat；默认 Java（带 jar 名） |
| python.exe、pythonw.exe | uvicorn / fastapi / flask / manage.py runserver、django / http.server | FastAPI (Uvicorn) / FastAPI / Flask / Django / Python HTTP Server；默认 Python |
| 数据库与中间件 | mysqld / redis-server / postgres / mongod / nginx / httpd | MySQL / Redis / PostgreSQL / MongoDB / Nginx / Apache httpd |

**特殊占用者**：

| 条件 | kind | 可操作 | 说明 |
| --- | --- | --- | --- |
| PID 0 | `idle` | 否 | TIME_WAIT 等记录由内核持有 |
| PID 4 | `system` | 否 | 80/443 等多为 HTTP.sys，445/139 为 SMB |
| 自身 PID | `self` | 否 | 不能结束 port-helper |
| lsass / csrss / wininit / winlogon / smss / services / svchost 等 | `critical` | 是，需二次确认 | 后端要求 `confirmCritical = true` |
| com.docker.backend / com.docker.proxy / vpnkit / dockerd / wslrelay / wslhost | `forwarder` | 是，前端先提示 | 建议 `docker stop` / 在 WSL 内处理 |
| 托管 Windows 服务 | `service` | 是 | 建议在“服务”中停止 |

### 6.4 进程操作（ops.rs、console.rs）

**身份校验**：每次操作前只读取目标进程的映像名与启动时间（不加载全量快照）。启动时间与入参 `startTime` 不一致 → `PROCESS_CHANGED`；进程不存在或已退出 → `PROCESS_NOT_FOUND`。

**正常关闭**（Windows 没有 SIGTERM）：

```text
校验身份/保护规则
  ├─ 进程有可见顶层窗口 → 对每个窗口 PostMessage(WM_CLOSE)       method = windowClose
  ├─ 否则 → 启动辅助进程 `port-helper.exe --ctrl-c <pid>`        method = consoleCtrlC
  │         （DETACHED_PROCESS，无控制台；AttachConsole(pid) → 屏蔽自身 Ctrl 处理
  │          → GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) → 退出码 0 表示已发送）
  └─ 两者都不可用 → outcome = unsupported（前端直接询问是否强制结束）
等待退出：WaitForSingleObject(timeoutMs) → exited / timeout
```

- 用辅助进程发送 Ctrl+C，是为了不影响 port-helper 自身的控制台（开发模式下有控制台）。
- `CTRL_C_EVENT` 会发给同一控制台上的全部进程，效果等同于用户在该终端按 Ctrl+C。
- `timeoutMs` 由设置项决定（默认 5000，限制 1000–60000）。

**强制结束**：`OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE)` → `TerminateProcess` → 等待最多 3 秒确认退出；`ERROR_ACCESS_DENIED` → `ACCESS_DENIED`。

### 6.5 权限（privilege.rs）

- `get_privilege`：`OpenProcessToken` + `GetTokenInformation(TokenElevation)`。
- `relaunch_as_admin`：`ShellExecuteW("runas", 当前 exe)`；返回值 `SE_ERR_ACCESSDENIED`(5) 视为用户取消 UAC → `UAC_CANCELLED`；成功后 `app.exit(0)`。

### 6.6 内存与运行时

- 窗口最小化时把 WebView2 的内存目标级别设为 Low，还原时设回 Normal（`memory.rs`，需 WebView2 Runtime 114+）。
- 自定义 Tokio 运行时：2 个工作线程、阻塞线程池上限 4 个。

### 6.7 日志

使用 `log` + `tauri-plugin-log`：开发环境输出到控制台；Release 只记录 Info 及以上，且不记录命令行全文（可能含密码、Token）。

## 7. 前端设计

### 7.1 技术与约定

- React 19 + TypeScript（strict）+ Vite；Tailwind CSS 4（`@tailwindcss/vite`）；shadcn/ui（Button、Dialog、Switch、Sonner）；lucide-react 图标。
- 颜色全部来自 CSS 变量，变量名与设计稿一致（`--bg`、`--surface`、`--text-primary`……），浅色写在 `:root`，深色写在 `.dark`，并映射到 shadcn 所需的 `--background`、`--primary` 等。
- 不引入路由库：侧边栏切换 `view` 状态（端口查询 / 进程反查 / 设置）。

### 7.2 页面与设计稿对应

| 页面 / 组件 | 设计稿画板 |
| --- | --- |
| `PortQueryPage`：空状态（大输入框 + 常用端口）→ 结果（紧凑输入框 + 结果列表 + 详情面板） | 01、02、04、05、06、08 |
| `PidQueryPage`：空状态 → 结果（按 TCP/UDP、端口分组 + 详情面板） | 10、11、08 |
| `SettingsPage` | 07 |
| `dialogs/*` | 12 |
| Toast（sonner，`position="top-center"`，标题栏区域） | 09 |

### 7.3 进程操作状态机（useProcessActions）

```text
idle
 ├─ 点“关闭进程” → [forwarder?] 转发进程确认 → closeConfirm → closing(倒计时)
 │       closing ─ exited  → Toast 成功 → 重新查询
 │               ─ timeout / unsupported → closeTimeout → 选择强制结束 → killing
 ├─ 点“强制结束” → [critical?] criticalConfirm（输入进程名） : killConfirm → killing
 │       killing ─ exited → Toast 成功 → 重新查询
 ├─ 点“立即释放端口” → releaseConfirm → 依次对每个进程走 closing → 结束后重新查询
 │       若释放后端口又被占用 → Toast 警告“又被 xxx 占用”
 └─ 任意错误 → 按错误码映射 Toast（ACCESS_DENIED 带“以管理员身份重新启动”）
```

关键进程在“关闭”时同样要先过二次确认。

### 7.4 设置与常用端口（tauri-plugin-store，`settings.json`）

| key | 类型 | 默认值 |
| --- | --- | --- |
| `theme` | `"light" \| "dark" \| "system"` | `"system"` |
| `listenOnly` | `boolean` | `true` |
| `closeTimeoutSec` | `number`（1–60） | `5` |
| `recentPorts` | `number[]` | `[]` |

常用端口算法（纯函数 `pushRecentPort`）：查询成功发起后，把端口移到数组首位、去重、截断为 10 个。主题为 `system` 时监听 `prefers-color-scheme` 实时切换。

### 7.5 标题栏

`decorations: false`；`TitleBar` 用 `data-tauri-drag-region` 拖动，按钮调用 `getCurrentWindow().minimize() / toggleMaximize() / close()`；capabilities 只开放这几项窗口权限和 `store:default`。

## 8. 测试与验证

| 层级 | 内容 |
| --- | --- |
| Rust 单元测试 | 端口/PID 校验、状态映射、查询组装（过滤、分组、隐藏统计）、应用识别规则、特殊占用者规则 |
| Rust 集成测试 | 测试内绑定随机端口（`TcpListener` IPv4/IPv6、`UdpSocket`），用 `net` + `query` 查询，断言 PID = 当前进程 |
| 前端 | `tsc` 类型检查 + `vite build`；`pushRecentPort` 纯函数逻辑 |
| 手工 | 按 todo §16.3 场景（Vite、Spring Boot、MySQL 服务、Docker、IIS）在真机运行 |

## 9. 已知限制

- 以普通权限运行时，SYSTEM 进程的工作目录、部分命令行可能读不到，界面显示“需要管理员权限才能读取”。
- `CTRL_C_EVENT` 会作用于同一控制台的全部进程；不响应 Ctrl+C 的进程会走“超时 → 强制结束”。
- 常用端口上限（10）与首次使用是否预置默认端口仍待确认，当前实现为不预置。
