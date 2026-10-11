# CLI 入口

中文 | [English](README.md)

`@wombat/cli` 读取参数，输出文本或 JSON 结果。CLI 通过类型化查询连接本机内核，并按请求启动 Web。

## 公开入口

- 包导出参数解析和 `runUsageCli`；`./format` 导出文本格式化功能。
- `api` 列出本机方法和 Schema；`call` 从 stdin 读取一个有大小限制的 JSON 请求。
- `web` 启动本机 Web 宿主。不指定子命令时，CLI 输出用量文本。

## 限制

JSON 结果写入 stdout，进度写入 stderr。用户决定与检查结果分别保留。未知存储格式需要按明确的恢复流程处理；不要删除原数据。

## 延伸阅读

- [CLI 指南](../docs/guides/cli.md)：命令、筛选、预算和错误。
- [CLI 参考](../docs/reference/cli.md)：发现原则、退出码、耗时和格式恢复。
- [CLI 指令](AGENTS.md)：维护和验证规则。
