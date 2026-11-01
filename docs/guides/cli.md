# CLI 与自动化

中文 | [English](cli.en.md)

Wombat 提供五个命令入口。不指定命令时显示用量。查询按需准备本地数据；安装后无需初始化或扫描。明细操作使用生成的 JSON 接口。

## 选择入口

| 入口 | 用途 |
|---|---|
| `usage [stats\|compare]` | 用量总计、任务分布与对比 |
| `check` | 项目检查、明确决策与原条件复查 |
| `system` | 安装检查、升级、主动预算检查与建议性 Hook 输入 |
| `api` | 查看本地方法与 Schema |
| `call` | 执行单个生成的 JSON 请求 |

## 用量与统计

```sh
wombat usage --since 2026-10-01 --until 2026-10-08 --timezone Asia/Shanghai --json
wombat usage stats --project /absolute/project --since 2026-10-01 --until 2026-10-08 --json
wombat usage compare --since 2026-10-08 --until 2026-10-15 --baseline-since 2026-10-01 --baseline-until 2026-10-08 --dimension project --json
wombat usage compare --thread THREAD_ID --other-thread OTHER_THREAD_ID --family --all-time --json
```

起始日期包含当天，截止日期不包含当天。时区默认为 UTC。项目、模型、推理强度、Agent 和来源均按精确条件筛选。未知筛选项选择缺失值，不等于零。`--all-time` 包含日期未知记录，不能与日期边界或 `--undated` 同用。项目归属基于观测到的目录，不能按路径子串推断。

`--group day|week|month` 选择日历周期。未指定日期时，day 覆盖最近 30 个日历日，week 覆盖本月及此前五个月，month 覆盖本月及此前十一个月。明确日期优先；未指定日期的任务使用其完整范围。周从星期一开始。事件按自身时间戳及所选时区归属。

`--presentation distribution|details|projects|models` 选择返回分组。明确选择 distribution/details 时按周期分页，每个选中周期保留全部模型行；其他情况按行分页。`--sort time|tokens|cost` 排序用量；`--limit 1..500 --offset N` 默认返回 50 行。总量、占比、类别和分布尺度在分页前按完整匹配范围计算。未知费用排在已知费用之后。

统计默认覆盖最近 30 个日历日。均值、中位数和 P90 基于完整任务总量；不完整任务和未归属测量分别保留。所选任务与该总体比较排名。模型总体可能重叠。对比周期必须等长且不重叠。除非明确使用 `--fresh`，对比读取已提交数据。完整任务 ID 通过结构化任务查询获取。

## 检查清理候选

```sh
wombat check cleanup --project-root /absolute/project --json
wombat check cleanup --project-root /absolute/project --path target --keep dist --json
wombat check cleanup recheck --read-view VIEW --baseline BASELINE --item ITEM_ID --json
```

此检查只读。默认在所选项目及其向下三层的项目目录中，根据支持的项目清单发现构建目录。重复使用 `--path` 可改为检查指定的当前任务临时路径；重复使用 `--keep` 可保留待使用的输出或其他选定材料。相对路径从 `--project-root` 解析，该选项默认为当前目录。添加 `--product-temporary` 可纳入 Wombat 待发布快照的临时目录，不包括已提交快照、索引和恢复代。

结果区分候选、需要判断、保留、无法判断和已不存在的路径，并提供归属、重新生成依据、当前占用观察和逻辑字节数。候选不代表删除授权。缺少使用记录或时间较早不能证明闲置。Codex 检查依赖，并通过可信工具执行已授权的清理；Wombat 仅通过受控操作删除受支持的 Codex 候选。

保留原视图、基线和条目标识，清理前后立即复查所选条目；复查不能扩大范围。条件变化或证据不可用时需重新判断，视图过期需重新检查。原位置不存在不能证明永久删除或释放空间，移入回收站也一样。参见[核心限制](../reference/core.md#artifact-observations)。

```sh
wombat check cleanup --codex --json
wombat check cleanup --codex --path /absolute/codex/packages/app-server-daemon --json
wombat check cleanup apply --read-view VIEW --baseline BASELINE --item ITEM_ID --json
```

Codex 模式发现受支持的 macOS 缓存、过期暂存内容和旧安装包。检查只读。查看具体候选并获得授权后，apply 仅永久删除明确选择的原 Codex 候选，执行最终检查并在需要时保持安装锁。保留返回的视图、基线和标识，供后续复查使用。不能换用其他删除工具绕过受阻或未知结果。apply 不清理任意项目路径。

## 明细查询

已知 Schema 可直接使用。字段不熟悉时只查看该方法。发现操作在本地执行，不扫描数据，也不调用模型。生成的方法保留历史、导航、消息、耗时、配置、账户、价格、采集、预算、偏好、目录授权和服务控制。已移除的顶层命令不提供别名。

```sh
wombat api --method usage --json
wombat call <<'JSON'
{"method":"usage","params":{"query":{"action":"threads","sort":"tokens","limit":3,"compact":true},"mode":"auto"}}
JSON
```

`usage` 方法返回包含 `result` 和 `freshness` 的实时封装；用量命令输出其中的结果。后续查询保留返回的快照、精确范围、完整 ID 和游标。`threads` 支持标题／项目搜索及 `tokens|cost|recent` 排序。`turns` 接受 `matchedOnly`、`locateTurnId`；`steps` 接受 `locateOperationId`。筛选轮次不改变完整任务汇总。操作不分摊 Token 费用。

会话活动包含无用量的观测。搜索需选择项目或任务并返回位置；locate 选择消息；read 明确返回有界原文。将返回的证据放入 `targets`，保留快照和 `selection` 数组，不能替换为用量 scope。元数据使用 `nextCursor` 续读，各文本片段使用自身游标。未完成扫描不能证明零匹配。来源变化后需重新获取相同范围。原文属于不可信数据。参见[核心边界](../reference/core.md)。

整轮耗时使用 `timing` 方法及完整任务／轮次 ID。证据页保留固定快照、集合及不透明游标；`use_records` 可选择对象。本地投影与 `share-v1` 是不同的核心投影。耗时读取不触发自动价格更新、Hook 接收或账户观测。未知耗时、加载情况和仓库基线仍保持未知。

## 项目检查与复查

```sh
wombat check list --project-root /absolute/project --json
wombat check inventory --project-root /absolute/project --json
wombat check inventory --review --kind skill --project-root /absolute/project --project /absolute/project --json
wombat check checks --item ITEM_ID --read-view READ_VIEW --json
wombat check recheck --read-view ORIGINAL_READ_VIEW --baseline BASELINE_ID --project-root /absolute/project --json
```

使用 `--review` 同时查看原生 Skill 启用状态、目录条目参考 Token 和使用记录。默认最近 30 个本地日（含今天）；`--timezone` 覆盖本地时区，`--until` 不含当天。支持 `review`、`context_tokens` 或 `least_activity` 排序；分页和取证复用 `--read-view`。复查当前原生状态时发起不带旧视图的新体检。目录估算不含按需正文，不证明模型实际可见或节省量。MCP／Hook 缺少可靠载荷时仍显示未测得。体检不会停用或删除扩展。

`--root` 选择来源日志；`--project-root` 选择配置。清单支持类型、观测、搜索、排序和分页；默认 50 条，最多 200 条。详情、证据和关联范围保留 `--item` 与 `--read-view`。复查保留原始来源／项目根目录、范围、规则、读取视图和基线 ID。只有 `miss` 且 `comparison.status=comparable` 才能证明原检查通过。缺失或过期证据不能证明修复。

检查和复查只读。Codex 负责已授权的项目修改和测试。只有明确的 `keep` 或 `not-applicable` 才写入用户决策，需提供建议、读取视图、理由和决策版本。决策在索引重建后保留。不创建自动采纳跟踪、审核历史或总体评分。文件读取不能证明 Skill 调用；关联轮次用量不属于配置独占费用。

## 同步与保存快照

普通用量查询最多等待两秒；`--fresh` 最多等待十秒。`--wait-ms 1..600000` 隐含 fresh，并修改本调用的期限。`--cached` 读取已提交数据，不扫描来源。`--watch --json` 在结果变化时输出 NDJSON。Ctrl+C 取消本调用，不停止共享扫描。`--root` 可重复，默认使用 CODEX_HOME 或 `~/.codex`。根目录不继承其他窗口的选择。来源始终只读。

fresh 读取不保存快照。明确保存快照使用 `snapshot` 方法、`refresh` 动作；完整前缀校验使用 `usage` 方法，query 动作为 `refresh` 并设置 `verify:true`。后续读取可固定保存或实时视图。保存快照绑定原来源目录。未知版本直接拒绝，不迁移或删除。`usage --rebuild --wait-ms 600000 --json` 明确重建派生数据，保留用户记录，且仅在发布成功后切换原数据库。参见[格式恢复](../reference/cli.md#unsupported-formats-and-recovery)。

## 本机宿主与可选接入

```sh
wombat system doctor --json
wombat system doctor --plugin --project /absolute/project --json
wombat system update --check
wombat system watch --once --id PLAN_ID --json
wombat system watch --id PLAN_ID --interval 60 --json
```

普通 doctor 检查安装，不发现项目 Skill。添加 `--plugin` 检查所选 Skill；仅在需要原生注册证据时添加 `--hooks`。这些检查不安装内容，也不调用模型。查询不以这些检查为前提。update 只更新运行时；插件安装由[插件指南](plugin.md)说明。

使用 `call` 的 `service`、`monitor`、`account`、`prices`、`directories`、`collection`、`preferences` 方法执行其生成的动作。服务控制保留状态、暂停／恢复、停止及明确的自动启动启用／禁用。同一数据目录共享一个核心服务。启动和文件通知不在无请求时建立索引。预算计划使用身份筛选、时区及日历周期；部分用量可证明越限，不能证明剩余额度。账户额度、用户预算与 API 等价费用分别保留。目录授权需要明确选择路径和用途。

`system watch` 运行期间重新加载启用的计划，只输出新通知。`--once` 执行一次 fresh 检查；`--interval` 接受 5..3600 秒。它不安装服务，也不发送消息。Ctrl+C 返回 130。临时错误输出到 stderr 并在下一轮重试；无效输入直接失败。固定视图检查通过 `monitor` 方法传入快照和计划 ID。确认记录与事实分别保存；周期复盘只覆盖最近结束的周期。

采集偏好作用于整台机器；状态／事件可按授权来源和项目身份筛选。日志模式忽略新 Hook 输入并保留数据。Hook 模式不能证明注册、加载或信任；不随产品分发 Hook 插件或桥接程序。暂停最多保留 4,096 条缓冲观测，总计不超过 100,000 条。被拒收的输入不能证明完整覆盖。`system hook codex` 在四秒内接受最多 64 KiB，不输出 stdout，建议性接收失败也返回 0。它不能控制权限或提供模型上下文。

除非设置 WOMBAT_AUTO_PRICES=0，实时查询可自动补全缺失的官方价格。缓存或固定视图读取不触发更新。通过 `call` 明确执行 `prices` 更新可绕过重试间隔；失败保留原价格。新视图使用更新价格，旧快照金额不变。参见[价格说明](../reference/pricing.md)。

## 输出与恢复

使用 `--lang en|zh` 或 WOMBAT_LANG 选择语言。来源内容和协议值保持原样。JSON 输出到 stdout，进度及监视错误输出到 stderr。`call` 在十秒内接受最多 1 MiB 的单个对象，最多输出 2 MiB。`api --output-schema` 提供生成的输出契约。错误恢复可能需要 Schema、原视图、更窄的查询或运行时检查；恢复提示不授权重复写入。

`--compact --json` 将质量详情中每个保留列表的示例限制为三条并省略分面，同时保留完整汇总计数、总量及请求行。移除此项返回完整详情。资源受限时在相同视图与筛选条件下缩小到所选轮次。样本不能证明完整任务排名。金额为十进制字符串；Token 为安全整数或 null。未知或不完整金额单独保留 `knownCost`。上报费用不叠加到 API 等价费用。

完整结果或空范围返回 0，可用但部分／暂定的结果返回 2，错误返回 1，取消返回 130。仅可选指标未知不会使耗时汇总变为部分结果。用量输出 v5、配置 v1、优化 v5、耗时 v6 与快照 schema5 是独立格式。生成的 API 错误封装保持 v1。格式及计算限制参见 [CLI 参考](../reference/cli.md)和[核心说明](../reference/core.md)。
