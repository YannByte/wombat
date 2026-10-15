# CLI 与自动化

中文 | [English](cli.en.md)

Wombat 默认增量同步本机 Codex 日志，查询增量更新；显式 refresh 另外保存固定快照。当前提供用量、任务、整轮耗时、配置测量与人工处理查询；所有子命令无需 TTY，JSON 与 Web使用同一 Rust 查询。

```sh
wombat prices --json
wombat prices update --json
wombat refresh --root /path/to/codex-home --json
wombat usage --since 2026-09-23 --until 2026-09-30 --timezone Asia/Shanghai --json
wombat threads --sort tokens --json
wombat turns --thread THREAD_ID --sort tokens --json
wombat steps --thread THREAD_ID --turn TURN_ID --sort time --json
```

`--root` 可重复；省略时使用 CODEX_HOME 或 `~/.codex`。同时读取 sessions 与 archived_sessions，显示范围不限制历史采集。刷新不接受查询筛选。


`wombat web --open` 在启动后请求系统打开浏览器；失败时继续运行并输出链接，不影响查询。

Hook清单JSON与Web详情提供同口径的项目注册观察及插件身份，文本输出也标明插件。注册不代表运行；支持的声明形式与未知状态见[配置契约](../development/contracts.md)。


## Agent 接口

用 `wombat api --json` 读取当前安装的方法、说明和预算，不扫描数据。用 `wombat api --method usage --json` 读取单个输入 Schema；需要响应字段时附加 `--output-schema`。两种 Schema 均从 Rust 生成，同一运行版本可以复用。

```sh
wombat api --method usage --json
wombat call <<'JSON'
{"method":"usage","params":{"mode":"auto","query":{"action":"threads","scope":{"project":"/absolute/project","since":"2026-10-01","until":"2026-10-08","timezone":"Asia/Shanghai"},"sort":"tokens","limit":3,"compact":true}}}
JSON
```

`call` 从标准输入接收单个 JSON 对象，默认输出 JSON，不交互提问。只派发清单中的生成协议方法，参数以所选 Schema 为准。结果保留所属协议，包括 `usage` 实时包装中的 `result` 和 `freshness`。下钻时保留快照、读取版本、范围、分页位置和游标；修改动作继续使用既有授权和目标选择要求。

输入上限为 1 MiB，标准输入时限为十秒；输出上限为 2 MiB，产品查询自身的预算仍然适用。错误返回 `{outputVersion:1,error:{code,message,recovery}}`，不回显原输入、路径或内部诊断。`recovery` 指示读取 Schema、重新取得原视图、缩小查询、重试同一范围、检查接入或核对状态，不代表可以自动重试写入。退出码为：完整结果 0、部分结果 2、错误 1、取消 130；对应的普通命令使用同一判断。Ctrl+C 取消本次调用，不停止共享扫描。

## 对比用量

周期对比与会话对比均读取已提交数据，默认不刷新。先执行 `wombat usage --fresh`，再使用下列命令；结束日期不含当天，两个周期须等长且不重叠。

```sh
wombat compare --since 2026-09-08 --until 2026-09-15 --baseline-since 2026-09-01 --baseline-until 2026-09-08 --dimension project --json
wombat compare --thread THREAD_ID --other-thread OTHER_THREAD_ID --family --all-time --json
```

`--dimension` 可选 `project`、`model` 或 `thread`；`--limit` 和 `--offset` 控制贡献分页。会话 ID 使用 `threads --json` 的 `id`；`--family` 加入明确关联的后代。两种比较均保留日期和维度筛选，并支持 `--snapshot` 固定版本；显式 `--fresh` 才请求同步。过期版本返回错误，重新读取列表后再比较。实时结果中的 `freshness.publicationChange` 说明最近成功发布的变化，文本也显示变化计数；没有完整基准时不提供摘要。[核心参考](../reference/core.md) 定义计算与限制。

## 任务分布与预算

用任务统计判断某个任务在所选总体中的位置，或把用量增长拆成任务数量变化与单任务用量变化。后续分页和比较保留返回的快照。

```sh
wombat statistics --project /absolute/project --since 2026-10-01 --until 2026-10-08 --presentation projects --json
wombat statistics --snapshot SNAPSHOT_ID --thread THREAD_ID --since 2026-10-01 --until 2026-10-08 --json
wombat statistics --snapshot SNAPSHOT_ID --since 2026-10-01 --until 2026-10-08 --baseline-since 2026-09-24 --baseline-until 2026-10-01 --presentation models --json
wombat monitor set --id project-week --period week --tokens 1000000 --review --project /absolute/project --timezone Asia/Shanghai --json
wombat monitor check --id project-week --json
wombat monitor watch --id project-week --interval 60 --json
wombat monitor list --json
wombat monitor acknowledge --notification NOTIFICATION_ID --json
wombat monitor remove --id project-week --json
```

任务统计默认选择最近30个自然日；`--all-time` 选择全部可用日期。项目或模型分组接受 `--limit` 与 `--offset`，总体覆盖完整范围。`--thread` 选择的任务与该总体比较。完整任务总量用于均值、中位数和 P90；不完整任务及未归属计量单独保留。一个任务使用多个模型时，各模型总体可以重叠。等长且不重叠的比较周期保留其他筛选。[核心参考](../reference/core.md)拥有计算方法与限制。

监控计划保存身份筛选、时区、自然周期及可选 Token 阈值。`--review` 同时请求最近一个已结束周期的统计。`--disabled` 保存暂停的计划；重复使用其 ID 可修改设置。预警默认在阈值的80%触发，可用 `--warning 0.9` 调整。这是用户预算，与账户额度独立。`check` 检查全部启用计划，`--id` 可选择一个；重复 `--root` 可选择来源。检查近期固定视图时使用 `--snapshot ID --id PLAN_ID`，不附加来源根目录。视图过期后，按原范围重新检查。

`watch` 在命令运行期间检查，只输出新提醒。它不安装后台服务或发送消息；Ctrl+C 以退出码130结束。`--interval` 接受5—3600秒。Web 使用同一组计划，支持手动检查，以及预算面板保持打开期间可选的60秒检查循环。监控需要运行中的宿主，来源日志记录用量后才能检测阈值。不完整事实可以证明越过阈值，不能证明剩余预算。确认状态与原始事实分开保存。周期复盘覆盖最近一个已结束周期，不补齐所有错过的周期。

## 使用检查与复盘

先读取或更新用量，再用返回的 `snapshotRef.snapshotId` 固定以下查询；默认仅读取已提交数据，不主动同步。

```sh
wombat investigate --snapshot SNAPSHOT_ID --all-time --limit 3 --compact --json
wombat investigate --snapshot SNAPSHOT_ID --thread THREAD_ID --turn TURN_ID --all-time --compact --json
wombat context --snapshot SNAPSHOT_ID --all-time --json
wombat trajectory --snapshot SNAPSHOT_ID --thread THREAD_ID --all-time --json
wombat resources --snapshot SNAPSHOT_ID --project /absolute/project --all-time --json
wombat review --snapshot SNAPSHOT_ID --since 2026-09-08 --until 2026-09-15 --json
wombat steps --snapshot SNAPSHOT_ID --thread THREAD_ID --turn TURN_ID --locate-operation OPERATION_ID --json
wombat account history --json
```

复盘不接受分页；省略日期时选择固定视图截止日期所在周，周一开始。其他检查可用 `--limit` 和 `--offset`，并保留同一范围。使用返回的完整证据身份下钻；操作不存在时返回 `NOT_FOUND`，视图过期时重新取得原范围并按身份定位。额度历史无需原生进程，只读取已保存的观测。阈值、统计口径及预算由[核心参考](../reference/core.md)维护；检查线索不能证明浪费，输入不是上下文占用，变更报告不是已验证的文件变化。

## Codex Skill

新版[用户 Skill](plugin.md)以对话任务为入口，可结合 Web 查看证据；处理继续在当前 Codex 对话中完成，Web 保留查看与复查。Skill 使用已有 CLI JSON，初始化复用有界索引恢复和同步，账户不等待日志。

正式插件由 Codex 管理，插件调用名在已核验的 Codex 0.160.0 中为 $wombat:wombat。独立本机试用调用名为 $wombat：

```sh
wombat skill install --json
wombat skill status --cwd /path/to/project --json
wombat skill install --replace --json
wombat skill uninstall --json
```

独立安装只从本机产品包的受审资源复制，默认 ~/.agents/skills/wombat；--directory 可指定以 wombat 结尾的目录，--cwd 指定原生发现项目。仅未改动的受管副本能明确替换或卸载；自定义目录、链接和用户改动受保护。JSON 使用独立 v1 契约，区分文件状态、发现/启用、运行能力和未请求数据。安装不扫描日志；原生发现无法确认时退出2，不回滚已经完成的文件安装。插件不由这些命令管理。

web --context FILE --json 接收受限生成契约：page 为 usage/threads/instructions/extensions/optimize，配对应 usage/configuration/optimization 只读请求。启动根仍由 --root/--project-root 授权；宿主读取并验证版本，返回有效 context 和连接链接。保留项目、来源、完整对象 ID、时区、日期与版本；CLI until 排他，URL 展示日由产品换算。日期必须成对或用 allTime，无法映射的浏览器筛选拒绝；浏览器采用自身分页大小。重启后须重新取得链接，连接令牌不外发。

需要独立 Codex 任务时，CLI 交接按项目检查实际启用的 Skill，发送时复核名称和路径，并在持久队列传入 text 与 skill 项。使用 --skill PROJECT_ID=PATH；缺失、禁用或冲突时须选择实例，或明确 --without-skill 沿用既有交接。送达未知不自动重发，接受请求不表示修改或复查已完成。Web 的主要界面不再提供派发按钮，既有交接接口与记录保留。详见[产品方案](../decisions/proposed/product/2026-10-04-codex-skill.md)。

## 语言

使用 `--lang zh` 或 `--lang en` 选择展示语言；也可设置 `WOMBAT_LANG`。JSON 字段与原始内容保持不变，完整优先级见[产品语言](../i18n/product.md)。

## 自动同步

- 普通查询默认同步，最多等2秒；有旧结果时返回并公开 `freshness`，尚无数据时返回 `SYNC_PENDING`。首次大目录建库可能需要更久。
- `--fresh` 等待本次同步，最长10秒；失败或超时明确报错。`--cached` 不触发来源扫描，返回已提交索引；没有索引时报错。
- `usage --watch --json` 输出逐行 JSON，版本、日期范围或状态改变时发出结果；Ctrl+C 退出130。普通 `--json` 仍只输出一个对象。
- `refresh --verify` 完整重读来源再保存快照，用于核验追加快速路径无法证明的历史前缀改写。正常 refresh 利用增量游标。
- `--snapshot ID`固定当前快照，后续分页继续传同一snapshotId，不能重新解析latest。未知格式明确拒绝。
- `--root` 可用于实时查询；每次省略时仍使用默认 Codex 来源，不会因为另一窗口指定根而改变。固定用量快照不能同时指定来源根；耗时查询的来源根用于将请求范围绑定到固定版本。

同一数据目录共用 Rust 服务。安装后的后台服务无需 CLI 或浏览器持续连接即可索引，`usage --watch` 只观察结果。计量以完整日志记录为准，模型尚未写入的 Token 无法即时显示。服务生命周期及资源限制归[内核参考](../reference/core.md)所有。

## 后台索引

```sh
wombat service status --json
wombat service pause --json
wombat service resume --json
wombat service stop --json
wombat service start --json
wombat service disable --json
wombat service enable --json
```

`status` 分别报告运行实例、准备进度、已提交数据是否就绪及自动启动配置。`pause` 在当前事务边界生效，仍可查询已有数据；`resume` 继续索引。`stop` 有界等待当前实例退出，后续查询或登录仍可能启动它。`disable` 移除当前用户的自动启动配置并停止服务，`enable` 注册并启动服务。自动启动需要受管安装及可用的原生用户会话管理器。显式限定来源根的 Web 宿主不能修改全局服务设置。Web 接入面板展示状态及允许的暂停或恢复入口。

## 整轮耗时

```sh
wombat timing --thread THREAD_ID --turn TURN_ID
wombat timing summary --thread THREAD_ID --turn TURN_ID --text --lang en
wombat timing --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID --share
wombat timing evidence --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID --limit 50
wombat timing evidence --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID --limit 50 --cursor OPAQUE_TOKEN
wombat timing capabilities
```

`timing` 与 `timing summary` 等价，默认输出一个最终 v6 JSON 对象；`--text` 选择本地化文本，不能与 `--json` 同用。使用任务查询返回的完整 Wombat 任务和轮次身份。缺失值在 JSON 中保留 null，文本说明所需记录或可用范围；原生耗时、派生耗时、原生 TTFT 与首条内容记录延迟分别展示。区间并集与累加可能重叠，不能相加。

摘要支持重复 `--root`、`--source`，以及 `--fresh` 或 `--cached` 之一。`--snapshot` 固定结果，不能与 `--fresh` 同用；固定版本同时指定来源根或来源时，必须匹配该版本的授权范围。后续证据页使用结果返回的快照身份；缺失或过期版本不会退回 latest。证据只支持本机投影，要求固定快照与相同目标和范围，接受 `--limit 1..200`（默认50）及原样传回的 `nextCursor.token`，不接受刷新模式。分页不改变整轮摘要。能力查询只接受输出、语言选项和可选 `--share`，不执行扫描；它报告解析器支持，不能证明某轮次实际存在这些字段。

`--share` 请求 Rust 独立的安全摘要投影，CLI 不从本机 JSON 删除字段拼成分享结果。耗时查询绕过自动补价、配置扫描、Hook 采集和账户观察，拒绝日期、Token、金额、offset、compare、watch 参数。达到资源上限时，结果仍可保留已验证原生标量，受影响的派生值省略并说明计算限制。

摘要在内核标明已检查范围完整时退出0，即使可选字段未记录；部分或暂定结果退出2。证据导航和能力查询成功时退出0，不据此证明整轮完整。错误退出1，取消退出130；JSON 错误使用 `{outputVersion:1,error:{code,message}}` 本地化安全模板，包括 NOT_FOUND 和 VIEW_EXPIRED，不输出来源路径或底层错误详情。状态写 stderr；Ctrl+C 取消本次调用，不终止共享同步。真实内核与浏览器验收使用 `corepack pnpm verify:e2e`；结果只覆盖所选平台与范围。

## 官方价表

实时查询发现可补齐的缺价会自动检查官方价表；失败保留现有结果并返回 `priceUpdate`。失败15分钟、成功24小时内不重复下载；设置 `WOMBAT_AUTO_PRICES=0` 可关闭自动联网，`--cached` 和 `--snapshot` 始终不触发自动更新。手动 `prices update` 不受自动重试间隔限制。

`prices`（或 `prices status`）离线查看当前完整价表；`prices update` 从固定官方地址联网下载并校验，默认输出简要结果，`--json` 返回 `outputVersion:1`、action、origin、updated、source、sourceHash、catalogHash和完整catalog。价格响应/错误版本独立于用量v5；错误仍为`{outputVersion:1,error:{code,message}}`，退出码1或取消130。常见失败包括PRICE_FETCH_FAILED、PRICE_SOURCE_CHANGED、PRICE_CACHE_INVALID、OUTPUT_LIMIT、TIMEOUT和UPDATE_BUSY。

更新成功后，下一次实时同步按新价表生成完整读取版本；执行`refresh`可另存快照，旧快照金额保留。更新不支持自定义URL、导入路径或用量筛选。网络范围、代理、支持模型表与保存规则见[价格口径](../reference/pricing.md#联网更新价表)。

## 筛选与分页

- `usage --group day|week|month`：省略日期时，day 默认近30个自然日，week 默认本月及之前5个月，month 默认本月及之前11个月，均截止今天（响应 until 为明天，不包含）。显式 since/until 优先，切换分组不改变手动范围；限定任务且省略日期时展示该任务全部范围。周一起始；按事件时间及所选时区归日。
- `--since` 包含起日，`--until` 不包含截止日；`--all-time` 包含日期未知，不能与日期或 --undated 同用。缺省时区 UTC；Web 使用系统时区。
- `--model`、`--effort`、`--project` 精确匹配；项目是已观察到的目录证据，不是路径子串。`--model-unknown`、`--effort-unknown`、`--undated` 分别筛选缺失模型、强度和日期，不能与对应具体值或日期范围同时指定。
- `threads --search TEXT` 搜索标题或项目，`--sort tokens|cost|recent`，按匹配用量或最近匹配计量排序；轮次与步骤使用 `tokens|cost|time`。
- `--snapshot ID`固定当前快照，后续分页继续传同一snapshotId，不能重新解析latest。未知格式明确拒绝。
- `usage --presentation distribution|details`：默认 details 保持分类明细；distribution 只返回时段小计。显式指定此参数时按日期组分页，limit 是时段数，明细保留该时段的全部模型行；省略时维持逐行分页。`--sort time|tokens|cost` 按日期倒序或消耗倒序；完整范围的 distribution 刻度、峰值筛选、未计价 Token 与金额占比在分页前计算，未知金额排在已知金额之后。
- `--limit 1..500 --offset N`，默认50。完整范围排序、金额、分类、占比均在分页前计算。

## JSON

`--compact --json` 将质量详情中保留的各列表限制为三个例子，并省略 facets；`quality.detailSummary` 保存完整问题数、来源数、分类计数及省略详情数。总量、请求行、分页、范围和快照保持不变；去掉 `--compact` 可读取完整详情。`--limit` 只控制返回分页，不减少整任务计算。遇到 RESOURCE_LIMIT 时，用 `turns --matched-only` 选择轮次，再以同一快照与筛选检查该轮。单轮发现不代表整任务排行。


用量使用 `outputVersion: 5`。成功对象包含 action、snapshotRef、scope、availableRange、summary、items、page、quality。运行时对生成 Schema 校验，未知参数拒绝。普通查询 stdout 只有一个最终 JSON 对象，watch为NDJSON；状态说明写stderr。实时结果另有freshness，status区分current、syncing、stale、failed和fixed，checkedAt为最后成功检查时间；current只表示已处理本次观察到的日志范围。

金额为十进制字符串；Token 为安全整数或 null。`price.cost=null` 表示金额不完整，`knownCost` 为已知小计，status 区分 priced、partial、unknown。缺失不是零，reportedCost 不与标准折算相加。不能从已显示的两位金额重新求和。

任务返回 matchedUsage 与 threadUsage。轮次默认覆盖完整任务，matchedUsage 保留来处条件；turns 可加 --matched-only 仅返回有匹配计量的轮次，--locate-turn ID 定位其所在页（定位成功优先于 offset，否则使用 offset）。筛选轮次不改变完整任务汇总。轮次份额分母为完整任务，步骤份额分母为完整轮次。操作没有独占计量，不显示费用。`unassigned` 承载任务内未归轮记录。

退出码：0 成功或空范围；2 有结果但读取不完整或未确认同步完成；1 错误；130 取消。错误结构为 `{outputVersion:5,error:{code,message}}`，常见 code 包括 INVALID_ARGUMENT、NO_SNAPSHOT、SOURCE_UNREADABLE、SNAPSHOT_CORRUPT、UNSUPPORTED_VERSION、UPDATE_BUSY、CANCELLED、RESOURCE_LIMIT、DETAIL_UNAVAILABLE。部分结果仍可用返回的固定快照继续查询。

## 旧版迁移

仅读取当前 usage-v4 快照（schema4）；用量 JSON v5 与耗时 JSON v6 是独立输出格式。未知版本拒绝并保留已有文件和用户记录。不提供迁移或旧命令/输出兼容。已退出的scan/report/checkup/quota/codex/observe/compare命令不留占位入口。

## 本机 Web

执行 `wombat web` 并打开输出链接。`--port 0` 默认自动选端口；可重复 `--root <目录>` 限定来源，`--lang zh/en` 选择初始语言，`--json` 输出一行启动信息。Ctrl+C 停止此 Web 宿主，关闭标签不退出 CLI。

仅本机可访问，重启后须打开新链接。页面提供用量、任务、轮次、来源与价表；日期、模型、强度、目录筛选同步写入地址。浏览器刷新会重新读取本机版本，过期版本可点更新数据恢复。既有业务接口已通过 HTTP 打通，原始日志仍只读；不支持远程部署。详情见[架构](../development/architecture.md)。

新增查询：`usage --presentation projects|models` 按历史目录或模型归组；`--project-unknown` 筛选无目录证据记录，`--agent` / `--source` 限定来源，`threads --locate-thread ID` 返回完整 ID 所在页。任务排序按当前筛选匹配量/最近匹配计量；完整任务量仍独立返回。

## 只读配置

`wombat web --project-root /path/to/project` 添加历史中未出现的配置目录，可重复；省略时保留当前启动目录。本机 Web 会把当前来源中可靠的历史cwd自动加入项目目录，直接CLI配置查询仍用 `--project-root` 明确范围。`--root` 仍指定Codex来源，与项目配置根分开。

`wombat optimize inventory --json` 提供同口径无TTY查询。使用 `--kind rule|skill|mcp`、`--observation used|loaded_only|unknown`、`--search`、`--sort tokens|activity|size|name|content_tokens|characters|recent` 筛选排序；`--limit` 默认50、最多200，`--offset` 从0开始。`--since` / `--until` 为包含起日、不含止日，`--timezone` 默认UTC。

详情和证据使用 `--action detail|evidence|related_scopes --item ID --read-view VERSION`；创建版本时可传 `--snapshot live:…` 固定用量。继续查询须保留相同来源根和项目根参数；版本过期时重新查询清单。`--thread ID` 只列出该任务有证据关联的配置。`--action capabilities` 不扫描配置。完整参数见 `wombat optimize inventory --help`。

返回配置v1 JSON；当前历史覆盖不完整时退出码2仍包含可用结果，参数/服务错误为1，取消为130。文件读取不算Skill调用，关联Token不是配置独占费用，全文内容Token使用固定基准，静态建议与人工复查已接入，来源修改仍关闭。范围和数据含义见[配置契约](../development/contracts.md)。

## 静态建议与人工复查

检查一个轮次前，先从任务清单或耗时汇总取得快照、任务和轮次 ID。执行 `wombat optimize activity --snapshot SNAPSHOT_ID --thread THREAD_ID --turn TURN_ID --json`，保留所选视图的 `--root` 及可选 `--source`。文本输出显示三个检查结果及适用的检查建议。此查询不采集配置，不保存处理决定。查看记录时，使用相同选择查询 timing evidence。视图过期后重新打开清单并选择新视图，不混用不同快照的结果。

执行 `wombat optimize list --json`，传与配置相同的 `--root` / `--project-root`。建议已按对象合并，类别为 repair/trim/organize/space；后两类没有适配时明确不可用。检查不接受用量日期或模型，当前静态提醒不等于配置健康。

```sh
wombat optimize list --project-root /path/to/project --json
wombat optimize detail --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize keep --reason necessary --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize history --read-view READ_VIEW --json
wombat optimize redisplay --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize not-applicable --reason incorrect_evidence --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize recheck --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize capabilities --json
```

续查保留原授权根与项目/来源范围，每次使用返回的新decisionRevision；冲突重新读取。detail/keep/not-applicable/redisplay必须带建议身份，keep/not-applicable另需原因。列表支持category、offset及limit 1..200，默认50。在Wombat外修改后直接recheck，--suggestion可限制单项。复查不写来源或撤销用户决定，redisplay不恢复文件。配置/处理结果使用v1，证据不足退出2、错误1、取消130；能力查询不扫描，索引重建保留产品记录。

产品提醒值可用 `--agents-bytes 16384 --description-characters 500` 调整，后续操作保持相同参数。仅授权当前配置范围生效，记录保留参数。正文5,000参考线及description1,024规范上限不可修改；Web提醒值面板使用同一契约。全文/正文估算分别返回，见[契约](../development/contracts.md)。

Web优化列表显示建议、价值和关键指标，详情可展开处理步骤与依据，相关记录次数、轮次Token和API估算金额同屏。关联记录日期不改变当前静态检查；查看准确轮次后可返回原建议及原筛选。无关联保持未知，不能按次数分摊金额。CLI/Agent先从optimize结果取得item.id/readView，再用inventory --action evidence --item ID --read-view VERSION和日期/时区参数；由返回usageRevision/threadId/turnId定位turns。保留授权根，不混用读取版本。人工标记与复查结果通过reviewBaseline/item提供文本测量前后值；未知或不同方法不可比较，变化不代表节省。

## 本机 Codex 交接与账户

交接需要本机可运行的 Codex；当前原生接口在0.160.0验证。登录、模型审阅、执行及恢复均由 Codex 管理。CLI 和保留的 HTTP 交接接口使用同一清单；不会复制登录凭据。

```sh
wombat optimize handoff preview --project-root /path/to/project --json
wombat optimize handoff send --project-root /path/to/project --selection-version SELECTION_VERSION --read-view READ_VIEW --decision-revision REVISION --json
wombat account read --json
wombat account refresh --json
```

先审阅preview返回的项目工作目录、文件和依据，再使用返回的selectionVersion发送；保留相同来源、项目授权和筛选。省略`--suggestion`选全部待处理项，不受列表分页或类型限制；可重复指定该参数发送单项/子集。共享文件只出现一次，同文件规则合并；项目切换需要重新确认。发送前文件/依据变化明确拒绝。

发送返回accepted、failed或unknown及可用的Codex任务ID；accepted只表示接受请求。查看任务用`codex resume TASK_ID`。失联时先在Codex核对，再由用户决定重发；不会自动重发，不保存执行回执或遍历历史查重。重复手动发送可能产生新任务，关闭Wombat不取消已接受任务。处理后使用`optimize recheck`判断问题是否仍在。

低额度只提醒；当前任务有可靠的原生限制时阻止发送。过期、未知或其他模型的限制不当作当前任务耗尽，也不会自动重发。发送会按实际任务重新核对；最终受阻可能留下没有请求内容的空Codex任务。旧确认组件仍保留，但不属于当前 Web 的主要入口。

账户v1响应中的身份、额度和活动有独立状态与读取时间；仅显示脱敏邮箱。真实窗口名称、模型、周期和重置来自Codex，不固定五小时/七天，不回退旧单桶。失败保留先前数据和读取时间，换账户清除旧数据；已过重置时间不推定满额。余额和消费限额保留来源小数字符串，不猜单位；重置权益只读，未提供明细与空列表分开，明细上限128条且不替代来源总数。概览和账户详情共享读取结果，项目或日期不改变账户范围。额度不与项目Token/API估算金额相加或换算。部分读取及未确认交接退出2，错误1、取消130。近期轮次可用`turns --sort recent`，按可靠活动时间排序，未知时间置后。

## 接入与采集

`wombat setup --project /path/to/project --json` 检查原生发现与注册，分别保留各项状态。结果包含完整 Codex 版本、Wombat 运行版本，以及实际发现实例的能力声明和内容校验；文本输出也显示部分失败的错误码。查看日志无需信任 Hook；Skill 可用且兼容后，可在新的 Codex 对话中提问。此命令不安装、不信任、不读取账户凭据、不调用模型。`--root` 选择来源目录。缺少 Skill 不影响查看已有数据。

```sh
wombat collection status --json
wombat collection mode hooks --json
wombat collection events --project /path/to/project --limit 50 --json
wombat collection pause --json
wombat collection resume --json
wombat collection mode logs --json
wombat usage --watch --json
```

偏好作用于本机；`--project` 和可重复的 `--root` 只筛选状态与事件。logs 模式忽略新 Hook 输入并保留数据；hooks 模式允许安全接收，不证明注册或信任。安装[本地采集插件](plugin.md)，再在 Codex `/hooks` 中审查声明。POSIX 桥接使用受管启动器或 Codex PATH；Windows 仍未验收。移除插件不删除观察。

暂停最多保留 4,096 条安全观察，恢复后可用；历史仍可查看。最多保留 100,000 条观察，已知溢出和身份冲突计入缺口。缺少原生身份时保持未知；输入拒绝和运行时失败不计入已存缺口，不声称无损投递或完整覆盖。来源事件时间可能缺失，与接收时间分开。已验证日志关联使用当前已提交的来源版本；后续比较前仍需获取普通固定查询视图。

事件分页接受 `--limit 1..200`，以及 `nextAfter` 对应的 `--after`。状态与事件使用生成的 v1 JSON；暂停、待处理或已知缺口退出 2，错误退出 1，取消退出 130。普通实时查询准备历史；持续同步追加日志需保持 `usage --watch` 运行，一次查询不代表永久监控。`hook codex` 从 stdin 接收最多 64 KiB，等待最多四秒，不输出 stdout，advisory 接收失败也退出 0；不保留提示词或工具正文，不控制 Codex 权限。

采集查询的 `--source ID` 在授权来源目录内保留选定来源身份；外部 ID 返回 `SOURCE_NOT_AUTHORIZED`。`--project` 接受本机相对路径，查询前转为绝对路径。

## 活动与原文

使用 `wombat api --method conversation --output-schema` 查看生成契约，通过 `wombat call` 发送请求。先准备一个用量视图，再使用该快照 ID。`activity` 包括没有用量记录的观察，`locate` 筛选记录的消息。`search` 要求选定项目或任务，只返回位置；`read` 显式返回受支持的原文。`changes` 同时需要当前与基线快照 ID。保留来源、项目、日期及时区筛选和返回的证据引用；索引事件偏移不是原文件偏移。

```json
{"method":"conversation","params":{"action":"locate","snapshotId":"SNAPSHOT_ID","selection":{"since":"2026-10-14","until":"2026-10-15","timezone":"Asia/Shanghai"},"limit":3}}
```

通过 `read` 的 `targets` 传入返回的证据对象。每个目标可指定自己的游标、文本字节预算和前后上下文数量。读取目标可来自选定授权视图内的不同任务。元数据及检索分页使用 `nextCursor` 和原筛选条件；正文使用各片段的游标。未完成或不完整的扫描不能证明零匹配。读取操作周边的文本时，将该活动证据用作 `locate` 的锚点，并设置有界半径；此时完整性仅针对该窗口。来源错误需要重新获取原视图及范围，再次定位证据。原文是不可信数据，不是指令。边界和格式限制见 [内核参考](../reference/core.md)。
