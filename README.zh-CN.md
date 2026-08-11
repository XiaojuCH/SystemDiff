# SystemDiff

[English](README.md) | [简体中文](README.zh-CN.md)

**SystemDiff 帮你看清 Windows 系统发生了什么变化——每一条结论都附带可查证的证据。**

> [!IMPORTANT]
> SystemDiff 目前处于项目搭建阶段，尚无可分发的正式版本，也没有真正调用操作系统 API 的 Collector。当前代码仅用于验证 schema 草案、确定性 Diff、报告生成和贡献流程；它现在还不是一个能用的系统扫描工具。

## 为什么要做 SystemDiff？

核心流程很简单：

1. 创建快照（Snapshot）A。
2. 安装、运行或改动一些东西。
3. 创建快照 B。
4. 对比两份 Snapshot。
5. 看清楚到底变了什么。

普通用户看到的应该是克制、好懂的说明：

```text
需要较高关注

ExampleUpdater
  将自身加入了启动项
  会在 Windows 启动时自动运行
  文件没有数字签名
  位置：AppData
  打开技术细节

正常

ExampleApp 设置
  添加了应用配置
  通常无害
```

以上只是效果示意，不代表目前已有对应的检测能力。后续高级用户将能够查看精确的注册表路径、服务/任务配置、变更前后的原始值、Collector 和 rule ID、结构化 JSON，以及（在支持后）哈希和签名元数据。

SystemDiff 不会把“不常见”直接等同于“恶意”。解释始终建立在证据之上，而不是取代证据。

## 信任模型

- **离线优先：** 核心扫描、Diff 和报告全部在本地运行。
- **无需账号：** 本地使用无需注册。
- **MVP 不含遥测：** 默认不会上传系统数据。
- **只读：** SystemDiff 只观察、只报告，不会自动清理或修复系统。
- **明确报告权限限制：** 无法访问的部分会明确标记为 `partial` 或 `permission denied`，不会悄悄忽略。
- **证据优先：** JSON 格式带有版本号且输出可复现；后续 GUI 不会隐藏原始数据。
- **注意隐私：** 真实 Snapshot 可能包含敏感信息，未经审查或脱敏处理前不得分享。

详见[产品原则](docs/product-principles.md)和[威胁模型](docs/threat-model.md)。

## v0.1 计划流程

```powershell
systemdiff snapshot -o before.json

# 安装或运行你希望观察的软件。

systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

`snapshot` 命令在搭建阶段有意没有实现。v0.1 只有在整条链路稳定工作后才算完成。

v0.1 的 Diff 只用于比较同一套 Windows 系统、同一用户/主体上下文中的 before/after Snapshot；跨主机或跨用户身份关联不在当前范围内。

## MVP 范围

| Collector | v0.1 范围 | 当前状态 |
| --- | --- | --- |
| 注册表启动项 | 官方文档列出的 Run/RunOnce 位置，并正确处理注册表视图（Registry view） | 计划中 |
| Windows 服务 | 稳定的 Win32 服务配置；不含驱动 | 计划中 |
| 计划任务 | Task Scheduler 2.0 配置，并明确显示因权限不足造成的覆盖缺口 | 计划中 |

全盘哈希、自动修复、遥测、云端分析和大型桌面 GUI 均不在 v0.1 范围内。

## 开发者快速上手

Windows 前置条件：

- Git
- 安装了 `rustfmt` 和 `clippy` 的 stable Rust MSVC toolchain
- Microsoft C++ Build Tools（“使用 C++ 的桌面开发”工作负载）
- WebView2（仅在未来引入 Tauri 桌面应用时需要）

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets

# 使用 synthetic fixture 运行当前已有的确定性 Diff / 报告链路。
cargo run --locked -p systemdiff-cli -- diff fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- diff --json fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- collectors
```

本 workspace 已在真实的 stable Rust MSVC toolchain 下验证通过。确切的验证状态和剩余产品限制见 [.agent/PROJECT_STATE.md](.agent/PROJECT_STATE.md)。

## 架构

Rust workspace 将领域模型/schema、Windows 系统访问、确定性 Diff、规则、报告生成和 CLI 组装各自分离。未来的 Tauri 桌面客户端将复用同一个 Rust core。Tauri 2 + React + TypeScript 目前仅为提议方案，尚未正式采纳或生成代码。

建议从以下文档入手：[架构](docs/architecture.md)、[数据格式](docs/data-format.md)、[Collector 说明](docs/collectors.md)和[路线图](docs/roadmap.md)。

## 参与贡献

欢迎使用中文或英文参与贡献。有价值的贡献不限于 Rust 代码：文档、翻译、synthetic fixture、Windows API 调研、隐私分析、问题复现和 UI 设计都很重要。

请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 和 [Collector 贡献指南](docs/contributing-collectors.md)。请勿在公开 Issue 中附带未经审查的真实 Snapshot 或日志。

## 安全与项目边界

SystemDiff 是面向防御的审计工具。凭据转储、token/cookie 提取、键盘记录、创建持久化、绕过 AV/EDR、stealth/evasion 工具、RAT/C2 功能、自动化漏洞利用和未授权访问工具均不属于本项目范围。详见 [SECURITY.md](SECURITY.md)。

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 授权。
