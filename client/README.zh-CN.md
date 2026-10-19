# 类型化客户端

中文 | [English](README.md)

`@wombat/client` 为 CLI 和 Web 调用方提供同一个 `UsageClient`。Rust 定义业务契约；客户端校验请求和结果。

## 公开入口

- `@wombat/client` 导出 `createUsageClient`、类型和 `CoreError`。该入口不加载 Node 或终端库。
- `@wombat/client/node` 导出 `createNodeClient`，用于本机内核进程和共享服务。
- `@wombat/client/node/runtime` 为受管 CLI 安装启用可选的 Node 模块编译复用。缓存归属安装版本，不含查询数据，随版本删除。只读安装及 Node 缓存覆盖配置仍可正常使用。
- `@wombat/client/http` 导出 `createHttpClient`，用于本机 Web 宿主。
- `@wombat/client/locale` 提供展示语言支持。

## 限制

客户端只公开生成的操作，不提供任意 Shell 执行或通用文件写入。取消等待不代表写入没有发生。

## 延伸阅读

- [客户端参考](../docs/reference/client.md)：传输、超时、取消、接入和采集。
- [公开契约](../docs/development/contracts.md)：生成类型与独立版本。
- [客户端指令](AGENTS.md)：维护和验证规则。
