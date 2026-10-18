# 类型化客户端

中文 | [English](client.en.md)

`@wombat/client` 校验请求与结果，并向 CLI、Web 提供同一个 `UsageClient`。业务 DTO 来自 Rust 生成契约；[架构](../development/architecture.md)说明跨模块数据流。

## 公开入口

- 包根入口导出 `createUsageClient({ query, ...transports })`、类型与 `CoreError`。可选传输使用命名字段，包括 `timing`、`account`、`handoff`；根入口不加载 Node 或终端库。
- `@wombat/client/node` 的 `createNodeClient` 管理本机内核子进程、取消、超时和响应上限。
- `UsageClient.live`提供auto/fresh/cached实时查询与新鲜度；Node宿主管理共用本地服务，`query`保留固定快照接口。
- 可选的 `UsageClient.timing` 通过 Rust 生成的契约提供整轮耗时摘要、固定快照证据页和离线能力查询。未指定快照时，Node 选择实时读取视图；显式指定已保存快照时保持固定。耗时查询不触发价表下载、Hook 采集、配置扫描或账户观察。
- `UsageClient.prices`提供离线查询、显式更新和 `auto_update` 缺价检查；Node实时客户端自动触发固定HTTPS下载，Rust负责资格、持久限流、校验、保存及计价。
- 生成文件位于 `src/generated/`；字段和版本以 Rust DTO 及[契约](../development/contracts.md)为准。

- 展示语言使用 `@wombat/client/locale`，详见[产品语言与文案](../i18n/product.md)。

## 限制与验证

客户端只允许生成请求联合类型中的操作，不提供任意 shell、文件写入或通用 dispatch。请求与返回值在跨进程边界校验；修改此包时按[客户端约定](../../client/AGENTS.md)构建并运行聚焦测试。

Node 客户端和本地服务使用协议 2，并隔离接口地址与服务锁；协议 1 会被拒绝。普通实时读取默认等待最多 12 秒；显式 `refresh` 还需写入并同步固定快照，默认等待最多 120 秒。`waitMs` 可把单次内核同步等待设为最多 600 秒，强制读取默认仍为 10 秒；Node 传输额外留出 2 秒，`timeoutMs` 可覆盖客户端期限。耗时响应校验操作、隐私档位、方法，以及本机目标和固定快照身份。分享输出省略本机身份，由 Rust 绑定目标。取消实时等待会发送有界取消标记并拒绝迟到结果，内核释放该等待者，后台同步继续；已完成写入仍可能已生效。批量结果必须匹配请求动作和主读取版本。

只读操作在信号取消后，以 `CANCELLED` 拒绝迟到的传输结果。传输若已返回完成的偏好写入、目录授权变更、价表更新或交接发送结果，客户端保留该结果，即使取消信号迟到。取消等待不能证明写入没有发生。

本地套接字或原生进程访问失败时，传输在观察到操作系统拒绝后返回 `LOCAL_ACCESS_DENIED`，不返回原生诊断文本或路径。连接无法建立时，账户各部分保留这一原因；缺失值仍为未知。调用方可以申请必要范围的访问权限并重试同一读取，但传输不会自行授予权限或自动重发写入。

`UsageClient.service` 在 Node 和 HTTP 中使用 Rust 生成的版本 1 请求与响应。Rust 拥有就绪、进度、暂停状态及实例身份，Node 宿主拥有当前用户服务注册。受管启动器提供私有连接文件路径；客户端复用前检查协议、可执行文件身份和实例，安装后查询无需逐次启动内核进程获取连接地址。显式限定来源根的 Web 宿主仅返回裁剪后的服务状态，拒绝全局服务变更。读取状态不会启动已停止的服务。普通查询在已启用的原生注册与所请求数据目录匹配时重新启动该作业，否则启动按需进程。

## HTTP 传输

`@wombat/client/http` 导出 `createHttpClient({ origin, token })`，实现同一 `UsageClient`，校验结果并保留进度、取消和错误。浏览器不直接访问内核；仅向本机宿主发送窄操作。

本地 Web 宿主已实现 `/api/timing`；范围和版本授权见 [Web 宿主](web-host.md)。CLI 已提供耗时摘要、证据和能力查询，见[CLI 指南](../guides/cli.md)；共享耗时 UI 已接入。真实内核与浏览器联调使用 `corepack pnpm verify:e2e`，验收结果只覆盖所选平台与范围。

`UsageClient.config` 提供配置测量和证据，`optimize` 提供静态建议、用户记录和人工复查，`preferences` 只读取/保存 zh/en。配置对象和后续观察携带共用的类型化 `useBasis`，保留固定范围、方法、来源完整性、五维覆盖，以及不可用与已知零次的区别。`@wombat/client/locale` 为 CLI/UI 解释这些字段，不重算次数或推断历史采用。三者由 Rust 生成 v1 契约，Node/HTTP 并列实现；详见[公共契约](../development/contracts.md)。

`createNodeClient({automaticPrices:false})`只关闭客户端的自动补价装饰，显式prices操作仍可用。CLI实时查询默认保留原行为；Web使用此原始客户端并由宿主持有后台补价生命周期。

用户工作流复用通用入口的 WebViewRequest、validateWebViewRequest 和 webViewSearch；Node 入口另提供 manageSkill 与 discoverWombatSkill，分别管理独立副本和读取原生实例。公共状态来自 Rust；独立 Skill 安装不查询日志，受管运行时安装可以启动后台准备；详见[契约](../development/contracts.md)。

## 接入与运行时观察

可选的 `UsageClient.setup` 与 `UsageClient.collection` 使用 Rust 生成的版本 1 协议，校验输入和输出。Node 接入检查分别读取运行时能力、按 cwd 限定的原生 Skill 发现和 Rust 绑定的 Hook 注册，总时限八秒，保留部分失败代码。发现完成后立即检查运行时兼容性，较慢的 Hook 查询不会丢弃已完成的检查结果。内部超时返回 `SETUP_TIMEOUT`，用户取消仍返回 `CANCELLED`。接入检查只将当前清单与已提交用量核对，不调度日志采集；不安装、不信任、不读取账户凭据、不启动模型任务。Node `receiveCodexHook` 只通过私有内核进程边界发送有上限的原始输入，HTTP 不开放该能力。采集调用的进程时限五秒，输出预算 2 MiB。Web 宿主负责来源与项目授权，内核负责安全事实与关联。

接入检查返回完整 Codex 版本（含预发行与构建标识）、当前 Wombat 运行版本，并核对实际启用实例的内容哈希和能力声明。插件与运行时版本不同也可能兼容；版本相同不能单独证明兼容。无声明副本标为未受管，内容修改与能力缺失分别报告错误码。接入检查还返回已验证的运行时内置 marketplace 路径，用于生成显式原生安装命令。资源缺失保留为接入检查的部分错误；原生发现与采集事实仍可独立读取。

`UsageClient.monitor` 通过 Node/HTTP 提供类型化预算设置、显式检查、提醒及已读标记；实时检查在共用服务内使用所选快照。统计查询沿用 usage/snapshot 方法的 `action: statistics`。总体与持久化语义见[核心模块](core.md)，操作步骤见 [CLI 指南](../guides/cli.md)。
