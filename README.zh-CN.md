# SystemDiff

[English](README.md) | [简体中文](README.zh-CN.md)

**SystemDiff 用可检查的底层证据和清晰易懂的解释，告诉你 Windows 系统究竟发生了哪些变化。**

> [!IMPORTANT]
> SystemDiff 目前处于仓库 bootstrap 阶段，尚无面向最终用户的 release，也还没有任何真正调用操作系统 API 的 Collector。当前代码只用于证明 draft schema、确定性 Diff、报告与贡献边界；它现在还不是一个有效的系统扫描工具。

## 为什么需要 SystemDiff？

核心流程刻意保持简单：

1. 获取 Snapshot A。
2. 安装、运行或修改某项内容。
3. 获取 Snapshot B。
4. 比较两份 Snapshot。
5. 准确理解发生了什么变化。

普通用户应该看到克制、易懂的说明：

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

以上只是展示目标，并不代表当前已经具备相应检测能力。高级用户未来可以检查准确的注册表路径、服务/任务配置、原始 before/after 值、Collector 与 rule ID、结构化 JSON，以及在支持后查看哈希和签名元数据。

SystemDiff 不会把“少见”直接等同于“恶意”。解释建立在证据之上，绝不替代证据。

## 信任基础

- **Offline-first：** 核心扫描、Diff 和报告均在本地完成。
- **无需账号：** 本地使用不要求注册。
- **MVP 无 telemetry：** 默认不上传系统数据。
- **只读：** SystemDiff 负责观察和报告，不会自动清理或修复系统。
- **平稳处理权限：** 无法访问的范围会明确标为 partial 或 permission denied，而不是被隐藏。
- **Evidence-first：** JSON 格式有明确版本且输出确定；未来 GUI 不会隐藏原始证据。
- **重视隐私：** 真实 Snapshot 可能含敏感信息，未经检查或脱敏不得分享。

请参阅[产品原则](docs/product-principles.md)和[威胁模型](docs/threat-model.md)。

## 计划中的 v0.1 工作流

```powershell
systemdiff snapshot -o before.json

# 安装或运行你希望观察的软件。

systemdiff snapshot -o after.json
systemdiff diff before.json after.json
```

bootstrap 阶段刻意没有实现 `snapshot` 命令。只有这条完整链路能够可靠工作，v0.1 才算完成。

## MVP 范围

| Collector | v0.1 范围 | 当前状态 |
| --- | --- | --- |
| 注册表启动项 | 官方文档中的 Run/RunOnce 位置及正确 Registry view | 计划中 |
| Windows 服务 | 稳定的 Win32 服务配置；不包含 driver | 计划中 |
| 计划任务 | Task Scheduler 2.0 配置及权限感知的 coverage | 计划中 |

全盘哈希、自动 remediation、telemetry、云端分析和大型桌面 GUI 均不属于 v0.1 范围。

## 开发者快速开始

Windows 前置条件：

- Git；
- 安装 `rustfmt` 与 `clippy` 的 stable Rust MSVC toolchain；
- Microsoft C++ Build Tools（Desktop development with C++）；
- WebView2 仅在未来引入 Tauri 桌面端时需要。

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets

# 使用 synthetic fixtures 运行当前已有的确定性 Diff/报告链路。
cargo run --locked -p systemdiff-cli -- diff fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- diff --json fixtures/snapshots/before-v1.json fixtures/snapshots/after-v1.json
cargo run --locked -p systemdiff-cli -- collectors
```

本 bootstrap workspace 已使用真实的 stable Rust MSVC toolchain 完成验证。实际验证状态及仍未实现的产品能力记录在 [.agent/PROJECT_STATE.md](.agent/PROJECT_STATE.md) 中。

## 架构

Rust workspace 将 domain/schema、Windows 访问、确定性 Diff、rules、reports 和 CLI 组装分离。未来 Tauri 桌面端会复用同一个 Rust core。Tauri 2 + React + TypeScript 目前只是 Proposed 决策，尚未正式采纳或生成应用。

建议从[架构](docs/architecture.md)、[数据格式](docs/data-format.md)、[Collector 说明](docs/collectors.md)和[路线图](docs/roadmap.md)开始阅读。

## 参与贡献

英文或中文贡献都很欢迎。有价值的贡献不局限于 Rust：文档、翻译、synthetic fixtures、Windows API 研究、隐私分析、问题复现和 UI 设计都很重要。

请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 和 [Collector 贡献指南](docs/contributing-collectors.md)。请勿在 public issue 中附上未经检查的真实 Snapshot 或日志。

## 安全与项目边界

SystemDiff 是防御性审计软件。凭据转储、token/cookie 提取、键盘记录、创建持久化、绕过 AV/EDR、隐蔽/C2、自动化利用和未授权访问工具均不属于项目范围。详情参阅 [SECURITY.md](SECURITY.md)。

## 许可证

本项目使用 [Apache License 2.0](LICENSE)。在公开发布前，maintainer 仍应确认这一治理选择。
