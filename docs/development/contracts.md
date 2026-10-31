# 公共契约

中文 | [English](contracts.en.md)

Rust 的 `core/src/usage_app_dto.rs` 定义请求与响应，`adapters/contract.rs` 定义来源无关事实，`pricing.rs` 定义十进制金额与依据。

生成文件：

- [实时请求 Schema](../schemas/live-request-v1.schema.json)和[实时响应 Schema](../schemas/live-response-v1.schema.json)，源头为`core/src/live.rs`；`live`封装当前用量结果与独立新鲜度。
- [请求 Schema](../schemas/usage-request-v3.schema.json)
- [响应 Schema](../schemas/usage-app-v5.schema.json)
- [价表请求 Schema](../schemas/pricing-request-v1.schema.json)和[价表响应 Schema](../schemas/pricing-response-v1.schema.json)，源头为 `core/src/pricing_sync.rs`
- [监控请求](../schemas/monitor-request-v1.schema.json)与[响应](../schemas/monitor-response-v1.schema.json)来自 `core/src/monitor.rs`；计划、提醒和确认使用独立协议。
- `client/src/generated/usage-request.ts`、`usage-app.ts` 和校验器

运行 `corepack pnpm contracts:generate` 重生，`contracts:check` 拒绝漂移。通用客户端校验请求与响应，不提供通用 shell、任意文件写入或任意操作分派。

`outputVersion=5` 是公共结果版本，`schemaVersion=4` 是内部快照版本；来源适配器和价格各有独立版本。内核通信封装为 `{op:"usage_app",args:Request}` → `{ok:true,value:Response}` 或 `{ok:false,error,code,details}`。用量操作为刷新、用量、对话、轮次、步骤；独立价表接口为 `prices`，提供 status/update，响应 `outputVersion=1`。

公开操作、枚举、分页和错误详见[CLI](../guides/cli.md)。计量整数不能超过 JavaScript 安全整数；金额始终为十进制字符串。新字段及规则必须同时检查生成类型、CLI JSON 和当前快照。

用量汇总同时保留完整值与逐字段已记录小计，并给出筛选后规范计量记录的覆盖数和缺口原因；分页不改变范围。字段没有可用记录时小计为空，有效零值仍为零。完整用量份额不使用部分小计补分母。独立总量分析优先采用原生总量，仅对请求范围明确的响应，在原生总量不可用且原始输入与输出可靠时相加计算。原始输入已含缓存，输出已含推理；计算记录数和覆盖明确标注，原字段缺口保留，不还原累计区间。图表、排序和峰值使用该分析小计，份额要求同范围分析覆盖完整。计价仍采用原始测量。独立版本的可选分析保留不可变的原生观察审阅记录。公共类型、字段与枚举以 Rust 生成契约为准。

用量请求增加可选 `presentation`，默认明细，分布只保留时段小计；显式指定任一种 presentation 时 offset/limit/page.total 按时段计数，明细一页包含所选时段的全部模型行，不拆日期组；省略 presentation 维持原有逐行分页。`sort: cost` 与按分析小计的 Token 排序均由内核完成。响应 `distribution` 给出筛选范围内按分析小计计算的 Token 最大值、并列峰值日期及准确筛选 Scope、金额最大值和未计价 Token 数；用量、轮次和计量的 `costShare` 以完整范围已计价金额为分母，未知或零分母返回缺失。对话按 matchedUsage 或最近匹配计量排序，threadUsage 保留全量。轮次默认完整；仅 turns 接受 matchedOnly 与 locateTurnId，先筛选排序再定位分页。匹配轮次只改变 items/page，不改变全对话汇总和份额分母；步骤仍完整。

实时新鲜度initialScan表示仅包含首次扫描的临时任务元数据，来源partial且用量未知。预览不落盘，不属于无指定版本的cached结果；fresh/refresh仍等待完整同步。普通固定版本不变；边界见[首次任务视图决策](../decisions/implemented/architecture/2026-10-04-initial-task-preview.md)。


- [采集请求](../schemas/collection-request-v1.schema.json)与[响应](../schemas/collection-response-v1.schema.json)来自 `core/src/collection.rs`；偏好、安全接收与当前日志关联独立于 Token 账本。
- [接入请求](../schemas/setup-request-v1.schema.json)与[响应](../schemas/setup-response-v1.schema.json)来自 `core/src/setup_dto.rs`；宿主报告原生发现与注册，不将其等同于实际接收。

## 客户端入口

`@wombat/client` 导出生成的 `UsageRequest`、`UsageResult` 等类型，以及 `UsageClient.query(request, { signal, onProgress })`。`UsageClient.prices({action:"status"|"update"}, {signal,onProgress})` 返回生成的 `PricingResult`，含完整价表和版本/来源哈希。`UsageClient.live({query,mode,verify}, options)`为可选宿主能力；Node实现提供它，CLI默认使用，固定快照仍使用query。`createUsageClient(transport, pricingTransport, liveTransport, configTransport, optimizeTransport, preferencesTransport, directoriesTransport)` 包装受限传输并执行协议校验；其通用入口不包含 Node 或 React 依赖。

`@wombat/client/node` 提供 `createNodeClient({ binaryPath, timeoutMs, maxResponseBytes })`，负责本机子进程生命周期。调用方无需接触任意内核操作字符串。其他宿主可实现同一窄传输接口，并复用生成契约与校验器。

Node价表传输只下载固定官方HTTPS文档；内核`prices`操作的宿主请求在update时携带document，status不接受document。原始文档不出现在公共请求或响应类型中；Rust解析及发布，其他宿主需实现同一固定来源获取流程。

## 整轮耗时契约 v6

`core/src/timing_dto.rs` 定义类型化 `timing` 操作。生成契约包括[请求](../schemas/timing-request-v1.schema.json)、[响应联合](../schemas/timing-response-v6.schema.json)、[本地](../schemas/timing-local-response-v6.schema.json)与[分享](../schemas/timing-share-response-v6.schema.json)投影，以及[安全错误](../schemas/timing-error-output-v1.schema.json)。客户端导出 `TimingRequest` 和 `TimingResult`；字段以 Rust DTO 和生成 Schema 为准，不在此重复维护字段清单。

请求操作为 `summary`、`evidence` 和 `capabilities`。摘要要求完整任务与轮次身份，可选固定快照、来源范围、`auto`/`fresh`/`cached` 模式及 `local`/`share-v1` 隐私配置。证据页要求相同目标和固定快照；`turn_events`、`use_objects` 与 `use_records` 使用绑定视图和范围的不透明游标。capabilities 不扫描来源。摘要使用 `outputVersion=5` 和分析方法 `safe_event_turn_v7`；操作起止归并方法单独版本化，旧响应版本明确拒绝。本地结果可含本地身份与路径，`share-v1` 是单独的白名单投影，不包含这些信息。耗时查询不触发价表下载、配置扫描、Hook 或账户观察。命令及错误处理见 [CLI 指南](../guides/cli.md)。

## 只读配置契约 v1

`core/src/config_dto.rs` 生成[请求](../schemas/config-request-v1.schema.json)与[响应](../schemas/config-response-v1.schema.json)，Node 提供 `UsageClient.config`。list/detail/evidence/related_scopes/capabilities 不写来源；capabilities 不扫描。无 readView 时重采集。默认 UTC 近30天、截止日排他；`scope.allTime` 包含未知日期，不能同时给日期。配置范围不接受模型或强度；当前内容测量与日期无关。

readView 固定配置元数据和用量引用，全局最多8版、10分钟；淘汰、过期和重启返回 VIEW_EXPIRED。有效配置版本延长内核寿命并保留原用量，供返回任务轮次。续查 roots/projectRoots 必须与创建时一致。

来源根盘点准确文件名 AGENTS.md、config.toml 顶层 MCP、skills 和 .agents/skills；默认 ~/.codex 还读 ~/.agents/skills。授权项目中有界递归发现 AGENTS.md，读取 .agents/skills 与 .codex/config.toml；不进入 .git、node_modules、target、dist、.codex、.agents 或符号链接目录。大小写不敏感的文件系统仍核验准确目录项名。只提供发现事实，不推断加载优先级或历史采用。扫描单文件10 MiB、总读64 MiB、检查条目和新配置各10,000、深度8、问题256，超限公开 resourceLimited。

完整 UTF-8 文件提供真实字节数、Unicode 码点数和内容哈希；Skill 包含 front matter。`estimate` 用固定 `tiktoken-rs-0.12.0/o200k_base/ordinary-v1` 普通文本编码，注明全文、指纹与 referenceEncodingOnly，不包含聊天包装或等同未知模型。估算单文件1 MiB、单轮8 MiB，缓存最多2,048个计数，键包含方法和哈希，不存正文。空内容为零；超限、无效编码、缺失及读取失败保持独立状态。可靠元数据字节数声明 filesystemMetadata，不配旧内容值；历史或 stale 测量始终标为旧值。MCP 缺模型可见完整定义，字符和 Token 未知，不执行服务获取定义。

Hook单独测量当前声明原文：JSON对象、TOML平铺表头至末尾值或内联对象，保留原始转义和空白；不含其他配置、组级匹配器及引用脚本。bytesSource为hookDeclarationUtf8，estimate.payload为hookDeclaration，估算指纹绑定声明文本，item.contentHash仍绑定完整配置文件。无法可靠取得范围的嵌套TOML声明保持declarationUnavailable，不报配置无效；启用/信任/触发仍需要独立宿主证据。hooks.state是状态表，不作为事件。文件版本哈希每次读取仅计算一次，JSON借用声明原文，不复制脚本或命令正文进清单/缓存。

配置响应hookRegistry独立提供授权项目的原生注册观察：启用、信任、处理类型、声明文件版本及注册哈希，不表示运行或注入。按校验后的结构、来源身份和文件哈希绑定普通文件键、插件独立JSON及内联声明键。原生版本仅用于诊断，不用版本白名单决定是否接入；当前CODEX_HOME必须属于所选来源，项目必须显式授权，不用来源目录冒充工作目录。Node先有界预检本地Hook声明及显式启用的插件；两者均无时不启动原生宿主。预检另有每文件10MiB/总32MiB预算。Node在两次一致的hooks/list之间对授权范围内普通文件流式计算哈希，Rust再次匹配当前清单版本；进入视图时超过60秒、原生失败、必需字段无效、重复身份、未知键形状或路径越界保持未知/partial。最多64个项目、512条注册、每文件10MiB/总32MiB哈希读取、64KiB缓冲和5秒查询取消期限；文件系统阻塞不承诺硬实时。完整注册观察不持久缓存，不导出命令/任意报错文本；发现与用户决定保留复查所需的项目、注册哈希及文件版本。固定readView保持原观察，新读取和发送复核重新获取，时间戳不改变相同证据的配置版本。CLI 文本与 JSON 共享结果。Unix已开放下述静态命令子集的hookTarget检查；Windows命令及运行证据仍待补。

插件注册包含明确pluginId，键中的身份、包内相对路径、事件及位置必须一致；相对路径不得含空分量、点分量或反斜线，并须与授权普通文件的路径后缀精确匹配。只读取原生列出的独立JSON或plugin.json内联声明；内联单对象及对象数组按原生hooks[n]位置绑定，测量真实JSON指针处的处理声明。清单阶段每个物理文件只读一次，不猜测缓存布局或遍历插件目录。清单只纳入注册涉及的声明与授权项目，不把插件配置视为全局生效；安全文件元数据及最近项目范围可保留，注册身份/状态仍只在视图中。注册缺失但来源文件未成功读取时，保留最近声明为stale；明确文件不存在或成功读取后声明消失才转历史。文件版本变化须重新绑定；未知形状保持未知。CLI 文本显示插件身份，JSON使用生成契约。

hookTarget仅在当前文件版本匹配、原生明确启用且trusted/managed时检查；显式禁用可复查通过，缺注册/未信任/已变更保持未知。Unix使用锁定shlex2.0.1解析，不执行命令。插件命令采用Codex返回的已展开结果，包括原生插件目录/数据目录；正文仅经私有内核消息临时传入，单条上限16KiB、完整捕获上限768KiB，不进入公开DTO或缓存。结果须匹配项目、插件身份、注册哈希及当前文件的有效command声明；缺结果、版本变化或残留动态表达式保持未知，Wombat不自行展开变量。子集为python/python2/python3/python3.N（独立-B/-E/-I/-s/-S/-u/-q或--）、sh/bash/dash/zsh（独立-e/-u/-x/-v/-n或--，脚本名须含/）、无选项node/nodejs或--，以及带./、../或/的.sh/.py/.js/.mjs/.cjs直接路径。内联执行、模块执行、未知选项、环境赋值、管道/替换/通配/控制字符均未知。相对基址来自原生注册工作目录；按文件系统顺序检查授权边界及软链，不以权限错误或目录类型当缺失。Node缺入口时还排除.js/.json/.node扩展候选；目录或候选存在时保持解析未知。这里只断言静态引用，不证明解释器身份、脚本有效或运行失败。

分析仅重读所需声明文件，每文件10MiB/单轮32MiB，同文件只解析一次；只保留静态路径及最多512条注册的稀疏检查，不建清单×项目矩阵。建议绑定项目、原生键/哈希和文件版本；历史按原来命中的项目复查，其他项目的新问题独立待处理，缺原授权/注册不通过。交接工作目录从实际命中的授权项目选择；声明字数未知不阻止版本已核验的脚本引用交接。原生确认的父子项目继承只补充当前视图，不写配置缓存；原生暂时不可读时，当前授权对象的项目历史和决定仍保留，复查结果为未知。

Skill YAML 校验必填字段类型、名称及已核验目录匹配，选填 license/allowed-tools/compatibility 字符串和 metadata 字符串映射；未知宿主字段不直接报错。名称按[官方参考校验器](https://github.com/agentskills/agentskills/blob/main/skills-ref/src/skills_ref/validator.py)去两端空白、NFKC、Unicode小写/字母数字/连字符及64码点检查，compatibility≤500码点。description 长度只由字符串字段产生；头部64 KiB、事件10,000、深度32，无别名/include，重复键拒绝。超限或未支持标签不报格式错误。最近匹配来源/项目的原生Skill目录包含对象时显示最近可用；目录缺失不宣称禁用。正向采用声明或非失败的定向`SKILL.md`读取形成近似使用证据，同一任务轮次去重为usageCount，并另计相关任务和读取次数；未观察到保持缺失，不能解释为未使用或已遵循内容。日期仅筛历史活动证据，不改变最近目录状态；lastRecordAt为最后匹配活动。MCP工具尝试与明确资源读取仍分别计数，目录/模板查询不计使用，工具前缀不认定实例。

MCP 解析读取原生 mcp_tool_call_begin/end 和 McpToolCall 持久项，结果缺失保留未知。调用身份按任务/轮次隔离；明确分叉的同一上游轮次与调用归回有记录的祖先，追加和重启保留相同结果。资源请求只保留服务与操作类型，不保存 URI、参数或返回正文。保留名没有明确对应请求时操作类型未知；更多边界见[决定](../decisions/implemented/architecture/2026-10-04-mcp-runtime-evidence.md)。

关联用量按可靠关联轮次中匹配范围的计量去重；不分摊配置独占成本，各行可能重叠，汇总用并集。没有关联保留缺失，完整轮次量另查原用量版本。每次证据查询仍遍历安全事实，增量关联索引、持久复合版本及协作取消未实现；取消断开传输并隔离迟到结果，扫描受资源预算约束。

currentItems 不计明确缺失的路径，清单仍公开缺失行；显式读取以准确路径证据关联，不因 cwd 为授权根的子目录丢弃。

派生 config-v2 原子 JSON 只存安全元数据，每范围最多20,000项，旧缓存读取16 MiB。不存配置正文、环境值、命令参数或工具输出；不联网扫描，不执行 MCP。它可重建，与不可重建的用户处理记录分开。

## 优化与偏好契约 v1

`core/src/optimize_dto.rs`生成[请求](../schemas/optimize-request-v1.schema.json)与[响应](../schemas/optimize-response-v4.schema.json)。Node的UsageClient.optimize支持list/detail/history（group）、keep/not_applicable/redisplay/recheck/checks/capabilities/activity；CLI操作见[指南](../guides/cli.md)。对象级规则以本页阈值修订为准。日期及模型不影响检查，检查时刻独立返回。支持项目/来源/类别、pending/history、最多200项分页，默认50。

建议身份绑定来源、对象、内容指纹、问题与项目范围。readView固定事实，decisionRevision固定记录，冲突返回VIEW_EXPIRED。keep需原因necessary；not_applicable需object_changed或incorrect_evidence。用户决定保存真实时间，与规则结果独立；已确认重试不重复追加。redisplay只清除最新展示决定。recheck重采集当前范围，不撤销决定，返回stillNeedsReview、verified或recheckUnavailable及独立检查事实。verified只证明原问题对应规则的可观察检查通过，不证明采用或节省；历史事件保留不可变recordId与recordedAt。

响应followUps仅为本页verified记录派生后续观察，绑定recordId/suggestionId、原复查after、观察时刻与usageRevision，不写入处理库或重建处理时间。状态为no_observed_records、version_unknown或unavailable；没有观察时observedRecords为空，不补0。只核对原复查之后且不晚于本视图的可靠事件时间，保持项目、来源和物理对象身份；日期/模型/强度不改变此范围。明确文件读取与MCP工具/资源事件可关联，重放由原账本身份归并；Hook等缺运行适配或记录读取不可用时保持unavailable。文件读取不证明扩展调用，当前原生记录不带可靠历史内容版本，不能确认采用；absenceObservable始终为false。算法与边界见[来源事实决定](../decisions/implemented/architecture/2026-10-04-mcp-runtime-evidence.md)。

`checks` 按对象和规则独立返回 hit/miss/insufficient/unsupported/error，附规则/内容版本和检查时间；未产生建议不代表所有规则通过。`ruleCatalog` 公布规则与证据基础。完整 Skill 正文/指令的明确 Markdown 本地链接和图片引用核验存在性及文件/目录类型，保留原始字节与行位置；锚点、外部链接及代码/引用块不参与。授权边界外、动态路径、权限或预算缺口返回证据不足；引用变化进入配置读取版本，旧版本结果保持固定。文件内容不执行，也不因缺少原生宿主证据推断有效加载或闲置。

用户记录在 `user-v1/reviews.sqlite3`，独立事务、最多20,000事件，达到上限报错而不删除历史。索引重建保留用户决定。闲置、MCP故障及空间清理尚未支持。Wombat方案生成、批准应用、执行进度和恢复接口已移除，本机Codex交接已接入，未完成验收，见[未完成提案](../decisions/proposed/product/2026-10-03-optimization-lifecycle.md)。不暴露任意文件写入、shell或dispatch。

`core/src/preferences.rs` 生成[偏好请求](../schemas/preferences-request-v1.schema.json)与[响应](../schemas/preferences-response-v1.schema.json)，`UsageClient.preferences` 仅 get/set zh/en；私有原子文件为 `user-v1/language.json`，不接受任意路径或内容。展示行为见[语言契约](../i18n/product.md)。

`optimize` 响应版本 2 增加固定轮次的活动检查；请求版本 1 和持久复查格式保持原版本。`action: activity` 必须选择一个快照、任务和轮次，不接受配置视图或处理记录参数。它消费共用轮次分析，不采集配置、宿主钩子或价格，也不写复查历史。行为见[内核说明](../reference/core.md)，操作见[CLI 指南](../guides/cli.md)。无法识别的响应版本或字段会被拒绝。

当前规则使用static-config-v7。bodyTokenEstimate独立分词精确正文，payload=skillBody，含tokenizerVersion、method/contentHash及referenceEncodingOnly，所在configRevision/readView固定快照。bodyEstimateStatus公开未知/解析/资源缺口，旧缓存默认unknown；解析失败不写零。全文/正文各≤1MiB，两者输入都计入单轮8MiB预算。AGENTS.md>16,384B为产品提醒，Skill正文≥5,000为参考提醒，description501—1,024为产品提醒，>1,024为规范问题且不重复500提醒。ruleOverrides只允许agentsBytes正安全整数及descriptionCharacters 0..1024，ruleParameters返回默认/覆盖/固定线及授权当前配置适用边界；记录保留原规则，recheckRuleParameters保存复查规则。规范约束不可覆盖，正文未知不能复查通过。加载预算诊断/闲置/空间能力仍关闭，边界见[优化闭环提案](../decisions/proposed/product/2026-10-03-optimization-lifecycle.md)。

首次观察保存reviewBaseline：原始安全测量元数据，不含正文；后续复查保留基线并更新item。recheckRuleParameters记录实际复查参数。字节/码点需两端完整且当前；正文Token比较另须两端估算可用、方法/编码/载荷/分词器版本一致。仅展示文本变化，不将阈值差额或关联用量解释为节省。关联用量复用config evidence及usageRevision，日期不改变规则身份；CLI/Agent通过inventory evidence和turns读取同口径。

完整块与显式副本证据，公开文件版本、原文字节/行位置、声明哈希、方向和变换，不返回正文。config capabilities可返回启动授权根与宿主复制用重启命令；不是扫描成功事实，也不提供执行接口。live freshness.errorCode公开存储错误；facets.discoveredThreadCount只表示快照内已发现任务元数据，不是日期筛选后的计量数。

声明依据绑定来源实例、授权项目、声明路径/哈希、关系ID及种类；每条关系只有所有成员完整且适用条件已确认时才完成。标题或相对引用条件未知、声明消失、成员超限均为recheckUnavailable。相同声明文本不能替代其他项目的声明。

当前配置按本机规范路径、种类和原生键聚合物理对象，共享项目仅采集一次。authorizedProjects保留重叠根成员，project仅为展示归属；sourceContexts保留逐来源盘点身份、来源/版本及观察，sourceInstanceId仅为展示归属。来源筛选只计匹配事件；关联Token和金额仍用原账本并集。同一当前对象只产生一条建议，处理在所有匹配来源中一致，声明依据仍逐来源独立。操作只接受当前物理对象身份，不合并账本身份；文件在采集间改变版本时公开configContentChangedDuringScan并保留分项。

reviews.sqlite3仅使用user_version=4：review_events保存独立决定与检查状态，review_parts共享不可变依据、item、baseline及检查事实，内部采用SQLite JSONB。按授权对象/项目/类别COUNT及LIMIT/OFFSET后解码页面；状态只读轻量元数据，全局复查逐个解码历史对象。仅初始化空数据库，未知布局或版本拒绝，不迁移或删除。4MiB页缓存、512页检查点及8MiB WAL保留目标不是硬上限。共享依据与分页理由见[决定](../decisions/implemented/architecture/2026-10-02-rule-review-integrity.md)。


跨文件检查读取授权项目已有的 `.wombat/analysis.json`，格式见[生成 Schema](../schemas/analysis-declaration-v1.schema.json)。不创建声明或读取清单外文件；路径仅允许授权根内相对普通分量，双方须在完整当前清单内。chains仅用于Rule，identity-v1比较完整原文字节（含行尾），不证明原件正确或实际加载。单文件1MiB、单轮8MiB、32,768块、每组4,096位置、每对象256分支、输出证据2MiB；声明64KiB、256关系、每链2—64路径。超限公开缺口，不能完整复查通过；正文仅本轮有界驻留。

## 账户契约 v1

`core/src/account_dto.rs`生成[请求](../schemas/account-request-v1.schema.json)与[响应](../schemas/account-response-v1.schema.json)，CLI共用read/refresh。账户身份、额度、活动各有状态和读取时间；无凭据字段。Node读取前后身份，Rust检查账户归属并归一化；失败保留旧时间且标stale，换账户清除先前事实。

windows保留真实百分比、周期和重置；buckets分别返回原生credits、individualLimit和明确限制状态，不从百分比推定普通使用权限。余额及消费限额为校验后的原始小数字符串，来源未提供单位时不补货币或Token单位。异常字段标partial并保留合法同级字段，不使其他窗口变零。每次最多128桶；超限明确partial，不回退旧单桶。

resetCredits保留来源availableCount，不以明细数量替代；credits=null表示未提供，空列表表示返回空列表。明细最多128条，超限标detailsTruncated和partial。权益状态、授予/到期时间仅展示来源事实，没有兑换或重置接口。已过重置时间不代表恢复，旧数据不作为新鲜权限。windows/buckets的model来自normalModelSlug，只是额度别名的展示元数据，不表示任务使用该桶。modelRestriction只保留身份一致、已支持原生通知中的blocked_model_slug及reset_at；不保存通知文案、链接或动作。


## 交接额度判定

handoff响应的allowanceChecks逐项目返回状态、实际/预览模型、来源提供方、观察期限和原因。预览可选查询最多128个项目、并发4个、总预算5秒；未完成明确unknown，不阻止本地审阅。发送在thread/start返回实际模型/提供方后重新读取身份与额度，再次验证文件/依据后才入队。

仅ChatGPT账户、openai提供方及60秒内的原生明确模型限制可返回blocked：支持selected_model_limit和luna_reserve通知中的blocked_model_slug精确匹配，限制截止不晚于原生reset_at。过期转unknown，不推定恢复；无模型绑定的百分比、消费限制或展示名称不能拦截。普通codex桶剩余不超过10%只提醒；available仅表示原生报告普通账户额度可用，不保证当前任务能执行。UI按期限刷新状态，无自动发送；CLI文本与JSON使用相同结果。最终受阻可能留下已创建但未入队的空Codex任务，不另建回执或自动删除。

进入逐项目发送后，取消在入队前返回该项目 failed/CANCELLED，入队尝试后未确认接受则为 unknown/HANDOFF_UNKNOWN；尚未开始的项目为 failed/CANCELLED，不继续发送。更早取消可能直接返回 CANCELLED 错误。Node 可返回部分发送事实。取消不会撤回 Codex 已接受的请求。连接和本次发送保护会清理，重发仍需用户明确授权；不保存执行回执。

## Skill 契约 v1

core/src/skill_dto.rs 生成[安装响应](../schemas/skill-installation-v1.schema.json)和[原生发现](../schemas/skill-discovery-v1.schema.json)，Node 入口提供 manageSkill 和 discoverWombatSkill。文件状态 absent/unmanaged/modified/installed 与 available/ambiguous/disabled/missing/unavailable/selection_changed 的发现状态独立。dataStatus 为 not_requested，安装不查询用量或账户。


handoff v1 的 skillSelections 按 projectId/path 绑定，withoutSkill 仅由用户明确选择；响应 skillChecks 保留原生实例及选择，delivery.skillPath 记录实际绑定。原生名称可能含插件命名空间，队列接受确认同时核验消息身份和 Skill 名称/路径，未知不自动重试。
