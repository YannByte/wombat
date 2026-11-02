# 类型化客户端

中文 | [English](client.en.md)

`@wombat/client` 校验请求与结果，并向 CLI 提供同一个 `UsageClient`。业务 DTO 来自 Rust 生成契约；[架构](../development/architecture.md)说明跨模块数据流。

## 公开入口

- 包根入口导出 `createUsageClient({ query, ...transports })`、类型与 `CoreError`。可选传输使用命名字段，包括 `timing`、`account`；根入口不加载 Node 或终端库。
- `@wombat/client/node` 的 `createNodeClient` 管理本机内核子进程、取消、超时和响应上限。
- `UsageClient.live` 提供前台 auto/fresh/cached 查询及上次检查信息；`query` 读取当前用量事实。
- `UsageClient.timing` 提供有边界的完整轮次汇总和证据分页，不透明游标绑定目标及筛选条件；能力查询不扫描来源。
- `UsageClient.prices`提供离线查询、显式更新和 `auto_update` 缺价检查；Node实时客户端自动触发固定HTTPS下载，Rust负责资格、持久限流、校验、保存及计价。
- 生成文件位于 `src/generated/`；字段和版本以 Rust DTO 及[契约](../development/contracts.md)为准。

- 展示语言使用 `@wombat/client/locale`，详见[产品语言与文案](../i18n/product.md)。

## 限制与验证

客户端在边界校验生成的产品请求及结果。Node 使用有边界的前台内核子进程处理用量、会话、配置、检查、耗时、监控和清理，不发现套接字或启动服务。默认进程期限为 600 秒，宿主仍可设置 `timeoutMs` 和响应上限。取消会终止本次调用的进程树，并拒绝迟到的读取结果。取消或超时的写入可能已经提交；重试前应检查当前状态。

内核负责来源同步、SQLite 事务、游标有效性及基线比较，见[内部存储](core.md#foreground-queries-and-internal-storage)。最多八个附加用量查询与主请求共用来源范围及数据版本，整个批次不超过 2 MiB。`query` 读取当前用量事实；`live` 提供明确的 auto/fresh/cached 采集控制，均不接受公开视图选择器。

观察到操作系统拒绝原生进程访问时保留 `LOCAL_ACCESS_DENIED`，不会把原生诊断当作来源内容返回。传输层不会授予访问权或自动重放写入。修改后遵循[客户端规则](../../client/AGENTS.md)，执行生成契约及真实内核检查。

## 配置与耗时


CLI 提供耗时摘要、证据和能力查询。通过 `corepack pnpm verify:e2e` 验证所选平台的真实内核与 CLI。

`UsageClient.config` 提供测量与使用证据，`optimize` 提供只读检查、原条件复查和显式用户决定；`preferences` 读取或保存 zh/en。配置对象的 `useBasis` 保留固定范围、方法、来源完整性及覆盖。locale 只解释事实，不重算或推断采用。版本与字段见[公共契约](../development/contracts.md)。

`createNodeClient({automaticPrices:false})`只关闭客户端的自动补价装饰，显式prices操作仍可用。CLI实时查询默认保留原行为。


## 接入与运行时观察

`UsageClient.setup` 与 `collection` 使用 Rust 生成的版本 1 契约。setup 默认只核对 cwd 范围的插件发现、资源哈希和运行时能力，不采集来源日志。显式 `includeHooks` 才查询可选注册；八秒总时限内分别保留完成结果和错误。setup 不安装、不授予信任、不读取账户凭据、不调用模型。Node `receiveCodexHook` 将有界输入送往私有内核边界；采集调用时限五秒、输出上限 2 MiB，安全事实和关联归内核负责。

接入检查返回完整 Codex 版本（含预发行与构建标识）、当前 Wombat 运行版本，并核对实际启用实例的内容哈希和能力声明。交付的插件仅声明一个主 Skill，检查绑定其目录和内容哈希。插件与运行时版本不同也可能兼容；版本相同不能单独证明兼容。无声明副本标为未受管，内容修改与能力缺失分别报告错误码。接入检查还返回已验证的运行时内置 marketplace 路径，用于生成显式原生安装命令。资源缺失保留为接入检查的部分错误；原生发现与采集事实仍可独立读取。

`UsageClient.monitor` 提供预算设置、显式前台检查、通知及确认；统计查询使用用量入口。总体和存储定义见[内核](core.md)，操作流程见 [CLI 指南](../guides/cli.md)。
