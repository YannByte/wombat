# Rust 内核

中文 | [English](README.md)

`wombat-core` 读取本机 Codex 记录，创建固定快照。内核提供用量查询、计价、耗时分析、配置检查和预算监控。

## 入口

- `src/main.rs` 从 stdin 读取一个 JSON 请求，将最终结果写入 stdout，将进度写入 stderr。
- `usage_app_dto.rs` 定义公开请求和结果。工具根据 Rust 定义生成 TypeScript 类型和 Schema。
- `src/adapters/` 读取来源记录。扫描只写入 Wombat 产品数据。

## 限制

结果明确区分缺失值、未计价用量、部分读取和资源限制。Wombat 拒绝未知数据格式，并保留原数据。

## 延伸阅读

- [内核参考](../docs/reference/core.md)：算法、计量、存储、查询限制和服务生命周期。
- [来源验收](../docs/development/adapters.md)：支持的记录与独立测试预期。
- [开发流程](../docs/development/workflow.md)：构建和验证命令。
- [内核指令](AGENTS.md)：代码修改规则。
