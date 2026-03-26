# StarNet（星网）架构文档

## 项目概述

StarNet 是一款跨平台远程桌面控制软件，支持 Windows 和 macOS。通过 WebRTC 实现低延迟的实时屏幕共享与远程控制，采用硬件加速编码确保流畅体验。

### 核心目标

- **低延迟**：端到端延迟目标 < 100ms（局域网内 < 50ms）
- **跨平台**：Windows + macOS 双平台支持
- **安全**：端到端加密传输，可选穿透或中继
- **高性能**：硬件加速编码/解码，GPU 直接采集

---

## 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        StarNet System                               │
│                                                                     │
│  ┌──────────────┐     WebRTC (P2P)      ┌──────────────┐           │
│  │  StarNet     │ ◄════════════════════► │  StarNet     │           │
│  │  Client      │                        │  Host        │           │
│  │              │                        │              │           │
│  │ ┌──────────┐ │   Video (H.264)  ──►  │ ┌──────────┐ │           │
│  │ │ Client   │ │                        │ │ Host     │ │           │
│  │ │ UI       │ │   Input Events  ◄──    │ │ UI       │ │           │
│  │ │ (React)  │ │                        │ │ (React)  │ │           │
│  │ └────┬─────┘ │                        │ └────┬─────┘ │           │
│  │      │       │                        │      │       │           │
│  │ ┌────▼─────┐ │                        │ ┌────▼─────┐ │           │
│  │ │ Tauri    │ │                        │ │ Tauri    │ │           │
│  │ │ Backend  │ │                        │ │ Backend  │ │           │
│  │ └────┬─────┘ │                        │ └────┬─────┘ │           │
│  └──────┼───────┘                        └──────┼───────┘           │
│         │                                       │                   │
│  ┌──────▼───────────────────────────────────────▼───────┐           │
│  │              starnet-transport (WebRTC)               │           │
│  │         webrtc-rs / DataChannel                      │           │
│  └───────────────┬──────────────────────┬───────────────┘           │
│                  │                      │                            │
│  ┌───────────────▼──────┐  ┌───────────▼────────────┐               │
│  │ starnet-encode       │  │ starnet-capture        │               │
│  │ H.264 HW Encode/     │  │ DXGI Desktop Dup       │               │
│  │ Decode               │  │ ScreenCaptureKit       │               │
│  └──────────────────────┘  └────────┬───────────────┘               │
│                                     │                                │
│                             ┌───────▼─────────┐                     │
│                             │ starnet-input   │                     │
│                             │ SendInput /     │                     │
│                             │ CGEventPost     │                     │
│                             └─────────────────┘                     │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │              starnet-signal (Signaling Server)            │       │
│  │         axum + WebSocket  |  Port 8080                   │       │
│  │  Device Register / Pair / Offer / Answer / ICE           │       │
│  └──────────────────────────────────────────────────────────┘       │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │                 starnet-core                             │       │
│  │     Shared Types / Protocol / Serde / IDs                │       │
│  └──────────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 模块说明

### crates/starnet-core
**核心库** — 所有其他 crate 的依赖基础。

- `DeviceId` / `SessionId` — 设备与会话标识
- `ScreenConfig` / `CodecType` — 屏幕配置与编码类型
- `InputEvent` — 输入事件定义（鼠标、键盘）
- `ControlMessage` — 主控消息协议
- `SignalMessage` — 信令协议（WebRTC SDP 交换）
- 使用 `serde` 进行序列化，支持 JSON 传输

### crates/starnet-capture
**屏幕捕获** — 平台相关的屏幕采集实现。

- 定义 `ScreenCapturer` trait（start / capture_frame / stop）
- Windows: DXGI Desktop Duplication API（GPU 直接采集，零拷贝）
- macOS: ScreenCaptureKit（系统级高效采集）
- 输出原始像素数据（BGRA 或 NV12 格式）

### crates/starnet-input
**输入模拟** — 将远程输入事件转化为本地系统输入。

- 定义 `InputSimulator` trait（send_event）
- Windows: SendInput API（鼠标、键盘事件注入）
- macOS: CGEventPost（Core Graphics 事件模拟）

### crates/starnet-encode
**视频编解码** — 硬件加速的视频压缩与解压。

- `VideoEncoder` trait — 将原始帧编码为 H.264 码流
- `VideoDecoder` trait — 将码流解码为可显示帧
- Windows: Media Foundation / NVENC
- macOS: VideoToolbox
- 动态码率调整，按需请求关键帧

### crates/starnet-transport
**传输层** — WebRTC 数据通道抽象。

- 定义 `Transport` trait（connect / send_video / receive_input 等）
- 使用 webrtc-rs 实现原生 Rust WebRTC
- 自动 ICE 候选收集与连接
- 支持数据通道双向传输（视频帧 + 输入事件）

### crates/starnet-signal
**信令服务器** — WebRTC 连接建立的信令中转。

- Axum WebSocket 服务器，端口 8080
- 设备注册 / 注销
- 配对请求路由
- WebRTC 信令转发（Offer / Answer / ICE Candidate）
- CORS 支持，便于开发调试
- 后续可扩展 Redis 持久化

### apps/host-ui / apps/client-ui
**前端界面** — Tauri 2.0 + React + TypeScript。

- Host UI: 被控端界面，显示连接状态和设备信息
- Client UI: 控制端界面，显示远程桌面画面和操控面板
- Vite 构建，TypeScript 类型安全

---

## 技术选型理由

| 技术 | 选型 | 理由 |
|------|------|------|
| **主语言** | Rust | 内存安全、零成本抽象、优秀的异步生态 |
| **GUI 框架** | Tauri 2.0 | 轻量（< 10MB 安装包）、跨平台、Rust 原生集成 |
| **前端** | React + TypeScript | 生态成熟、类型安全、大量 UI 库可选 |
| **传输协议** | WebRTC | P2P 直连、低延迟、内置 NAT 穿透、加密传输 |
| **WebRTC 库** | webrtc-rs | 纯 Rust 实现，无 CGO/FFI 依赖 |
| **屏幕采集** | DXGI / SCKit | 平台原生 API，GPU 直接采集，最低开销 |
| **视频编码** | H.264 硬件加速 | 通用兼容性、硬件编码极低延迟 |
| **信令服务器** | Axum | 高性能、基于 Tower 生态、WebSocket 原生支持 |
| **序列化** | Serde (JSON) | Rust 标准序列化框架，调试友好 |
| **构建** | Vite | 极速 HMR、ESBuild 压缩 |

---

## Phase 1 路线图

### 目标：实现最小可用的局域网远程桌面

| 阶段 | 内容 | 预计时间 |
|------|------|----------|
| **1.1** | Core 类型定义 + 信号服务器基础 | 1 周 |
| **1.2** | Windows DXGI 屏幕捕获实现 | 2 周 |
| **1.3** | H.264 硬件编码/解码 (MF / VT) | 2 周 |
| **1.4** | WebRTC 传输层 (webrtc-rs) | 2 周 |
| **1.5** | Windows 输入模拟 (SendInput) | 1 周 |
| **1.6** | Tauri Host 应用集成 | 1 周 |
| **1.7** | Tauri Client 应用集成 + 渲染 | 2 周 |
| **1.8** | 端到端测试 + 性能调优 | 1 周 |

**Phase 1 总计：约 12 周**

### 后续阶段（展望）

- **Phase 2**: macOS 平台支持 + 自适应码率
- **Phase 3**: NAT 穿透增强 (STUN/TURN) + 云中继
- **Phase 4**: 多显示器支持 + 文件传输 + 剪贴板共享
- **Phase 5**: 移动端客户端 (iOS/Android)

---

## 延迟预算分析

端到端延迟目标：**< 100ms**（局域网 < 50ms）

### 各环节延迟分解

```
捕获    │ 编码    │ 打包    │ 传输    │ 拆包    │ 解码    │ 渲染
  8ms   │  5ms   │  1ms   │  5ms   │  1ms   │  5ms   │  5ms
        │        │        │        │        │        │
        ▼        ▼        ▼        ▼        ▼        ▼
     ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                    总计: ~30ms（局域网）
     ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

| 环节 | 预算 | 说明 |
|------|------|------|
| **屏幕捕获** | 5-10ms | DXGI Desktop Duplication GPU 直接采集 |
| **H.264 编码** | 3-8ms | 硬件编码器（NVENC / Intel QSV / VT） |
| **协议打包** | < 1ms | Serde 序列化 + WebRTC DataChannel 封装 |
| **网络传输** | 1-5ms | 局域网 RTT，WebRTC UDP |
| **协议拆包** | < 1ms | 反序列化 |
| **H.264 解码** | 3-8ms | 硬件解码器 |
| **渲染** | 5-10ms | Canvas / WebGL 绘制，含 VSync 等待 |
| **总计** | **30-50ms** | 局域网典型值 |

### 优化策略

1. **零拷贝链路**：DXGI 采集 → GPU 编码 → 发送，避免 CPU 拷贝
2. **低延迟编码配置**：`tune=zerolatency`，禁用 B 帧，调小 GOP
3. **自适应码率**：根据网络状况动态调整码率
4. **帧率优先**：在网络拥塞时降低分辨率而非帧率
5. **输入预测**：客户端本地立即响应，服务端确认修正

---

*文档版本: 0.1.0 | 最后更新: 2026-03-26*
