# 公共契约

中文 | [English](contracts.en.md)

Rust 的 `core/src/usage_app_dto.rs` 定义请求与响应，`adapters/contract.rs` 定义来源无关事实，`pricing.rs` 定义十进制金额与依据。

生成文件：

- [实时请求 Schema](../schemas/live-request-v2.schema.json)和[实时响应 Schema](../schemas/live-response-v1.schema.json)，源头为`core/src/live.rs`；`live`封装当前用量结果与独立新鲜度。
- [请求 Schema](../schemas/usage-request-v4.schema.json)
- [响应 Schema](../schemas/usage-app-v5.schema.json)
- [价表请求 Schema](../schemas/pricing-request-v1.schema.json)和[价表响应 Schema](../schemas/pricing-response-v1.schema.json)，源头为 `core/src/pricing_sync.rs`
- [监控请求](../schemas/monitor-request-v2.schema.json)与[响应](../schemas/monitor-response-v1.schema.json)来自 `core/src/monitor.rs`；计划、提醒和确认使用独立协议。
- `client/src/generated/usage-request.ts`、`usage-app.ts` 和校验器

运行 `corepack pnpm contracts:generate` 重生，`contracts:check` 拒绝漂移。通用客户端校验请求与响应，不提供通用 shell、任意文件写入或任意操作分派。

`outputVersion=5` 是公共结果版本，`schemaVersion=5` 是内部快照版本；来源适配器和价格各有独立版本。内核通信封装为 `{op:"local_query",args:{query:Request,mode:"auto"}}` → `{ok:true,value:Response}` 或 `{ok:false,error,code,details}`。用量操作包括用量、对话、轮次、步骤；独立价表接口为 `prices`，提供 status/update，响应 `outputVersion=1`。

公开操作、枚举、分页和错误详见[CLI](../guides/cli.md)。计量整数不能超过 JavaScript 安全整数；金额始终为十进制字符串。新字段及规则必须同时检查生成类型、CLI JSON 和当前快照。

用量汇总同时保留完整值与逐字段已记录小计，并给出筛选后规范计量记录的覆盖数和缺口原因；分页不改变范围。字段没有可用记录时小计为空，有效零值仍为零。完整用量份额不使用部分小计补分母。独立总量分析优先采用原生总量，仅对请求范围明确的响应，在原生总量不可用且原始输入与输出可靠时相加计算。原始输入已含缓存，输出已含推理；计算记录数和覆盖明确标注，原字段缺口保留，不还原累计区间。图表、排序和峰值使用该分析小计，份额要求同范围分析覆盖完整。计价仍采用原始测量。独立版本的可选分析保留不可变的原生观察审阅记录。公共类型、字段与枚举以 Rust 生成契约为准。

用量请求增加可选 `presentation`，默认明细，分布只保留时段小计；显式指定任一种 presentation 时 offset/limit/page.total 按时段计数，明细一页包含所选时段的全部模型行，不拆日期组；省略 presentation 维持原有逐行分页。`sort: cost` 与按分析小计的 Token 排序均由内核完成。响应 `distribution` 给出筛选范围内按分析小计计算的 Token 最大值、并列峰值日期及准确筛选 Scope、金额最大值和未计价 Token 数；用量、轮次和计量的 `costShare` 以完整范围已计价金额为分母，未知或零分母返回缺失。对话按 matchedUsage 或最近匹配计量排序，threadUsage 保留全量。轮次默认完整；仅 turns 接受 matchedOnly 与 locateTurnId，先筛选排序再定位分页。匹配轮次只改变 items/page，不改变全对话汇总和份额分母；步骤仍完整。

前台调用在完成同步后返回已采集事实。缓存读取使用 SQLite 中已提交的事实，不创建临时视图。


- [采集请求](../schemas/collection-request-v1.schema.json)与[响应](../schemas/collection-response-v1.schema.json)来自 `core/src/collection.rs`；偏好、安全接收与当前日志关联独立于 Token 账本。
- [接入请求](../schemas/setup-request-v1.schema.json)与[响应](../schemas/setup-response-v1.schema.json)来自 `core/src/setup_dto.rs`；宿主报告原生发现与注册，不将其等同于实际接收。

## 客户端入口

`@wombat/client` 导出生成的请求与结果类型。`UsageClient.query(request, options)` 读取当前用量；Node 提供 `live({query,mode,verify}, options)`，显式控制 auto/fresh/cached。两者均使用前台内核边界。`createUsageClient` 校验窄接口传输与结果。可移植入口不依赖 Node 或 React。

`@wombat/client/node` 提供 `createNodeClient({ binaryPath, timeoutMs, maxResponseBytes })`，负责本机子进程生命周期。调用方无需接触任意内核操作字符串。其他宿主可实现同一窄传输接口，并复用生成契约与校验器。

Node价表传输只下载固定官方HTTPS文档；内核`prices`操作的宿主请求在update时携带document，status不接受document。原始文档不出现在公共请求或响应类型中；Rust解析及发布，其他宿主需实现同一固定来源获取流程。

## 整轮耗时契约 v8

`core/src/timing_dto.rs` 定义类型化 `timing` 操作。生成契约包括[请求](../schemas/timing-request-v2.schema.json)、[响应联合](../schemas/timing-response-v8.schema.json)、[本地](../schemas/timing-local-response-v8.schema.json)与[分享](../schemas/timing-share-response-v8.schema.json)投影，以及[安全错误](../schemas/timing-error-output-v1.schema.json)。客户端导出 `TimingRequest` 和 `TimingResult`；字段以 Rust DTO 和生成 Schema 为准，不在此重复维护字段清单。

请求操作为 `summary`、`evidence` 和 `capabilities`。摘要要求完整任务与轮次身份，接受显式来源根、来源范围、`auto`/`fresh`/`cached` 模式及 `local`/`share-v1` 隐私配置。证据页要求相同目标和范围；`turn_events`、`use_objects` 与 `use_records` 使用绑定来源证据和范围的不透明游标。capabilities 不扫描来源。摘要使用 `outputVersion=8` 和分析方法 `safe_event_turn_v7`；操作起止归并方法单独版本化，旧响应版本明确拒绝。本地结果可含本地身份与路径，`share-v1` 是单独的白名单投影，不包含这些信息。耗时查询不触发价表下载、配置扫描、Hook 或账户观察。命令及错误处理见 [CLI 指南](../guides/cli.md)。


选定轮次的本地摘要包含来自起止归并器的有界操作冲突解释：原因码、操作身份和抽样原始证据位置。这些信息不另行计数，也不直接判定重试；分享结果不包含本地诊断。参数错误提供有界字段路径和固定校验原因，不回显被拒绝的值。

## 只读配置契约 v3

`core/src/config_dto.rs` 生成[请求](../schemas/config-request-v3.schema.json)与[响应](../schemas/config-response-v3.schema.json)，Node 提供 `UsageClient.config`。list/detail/evidence/explain/related_scopes/capabilities 不写来源；capabilities 不扫描。每次调用采集当前配置。默认 UTC 近30天、截止日排他；`scope.allTime` 包含未知日期，不能同时给日期。配置范围不接受模型或强度；当前内容测量与日期无关。

`explain` 在实际使用投影中按需记录判定，分页返回原始位置、计入或排除原因及数值贡献。紧凑基线分别绑定规范化输入、配置和范围，不依赖观察时间。重算使用当前规则，只有相同输入才支持规则结果比较；逐条差异明确披露明细页覆盖情况。不保存事件流水或中间状态历史。配置查询在解码前筛选存储中的计量分组，每条选中计量只关联轮次一次；内存事实直接借用，避免复制。 按配置类别查询时只发现所需类别，保留其他类别的缓存元数据；综合发现先读取明确配置位置，再进行宽范围指令查找。字节读取完成状态与操作归属缺口分开，普通元数据缺口不表示扫描未完成。解释结果为每个规范化操作保留最多四个原始位置，并展示最多 20 条来源级诊断及完整总数；归并阶段不作为多次使用。

复查接收原始选定发现、来源和项目范围及规则输入，重新比较当前证据，不保留视图，也没有视图过期。缺少证据不能证明修复。

来源根盘点准确文件名 AGENTS.md、config.toml 顶层 MCP、skills 和 .agents/skills；默认 ~/.codex 还读 ~/.agents/skills。授权项目中有界递归发现 AGENTS.md，读取 .agents/skills 与 .codex/config.toml；不进入 .git、node_modules、target、dist、.codex、.agents 或符号链接目录。大小写不敏感的文件系统仍核验准确目录项名。只提供发现事实，不推断加载优先级或历史采用。扫描单文件10 MiB、总读64 MiB、检查条目和新配置各10,000、深度8、问题256，超限公开 resourceLimited。

完整 UTF-8 文件提供真实字节数、Unicode 码点数和内容哈希；Skill 包含 front matter。`estimate` 用固定 `tiktoken-rs-0.12.0/o200k_base/ordinary-v1` 普通文本编码，注明全文、指纹与 referenceEncodingOnly，不包含聊天包装或等同未知模型。估算单文件1 MiB、单轮8 MiB，缓存最多2,048个计数，键包含方法和哈希，不存正文。空内容为零；超限、无效编码、缺失及读取失败保持独立状态。可靠元数据字节数声明 filesystemMetadata，不配旧内容值；历史或 stale 测量始终标为旧值。MCP 缺模型可见完整定义，字符和 Token 未知，不执行服务获取定义。

Hook单独测量当前声明原文：JSON对象、TOML平铺表头至末尾值或内联对象，保留原始转义和空白；不含其他配置、组级匹配器及引用脚本。bytesSource为hookDeclarationUtf8，estimate.payload为hookDeclaration，估算指纹绑定声明文本，item.contentHash仍绑定完整配置文件。无法可靠取得范围的嵌套TOML声明保持declarationUnavailable，不报配置无效；启用/信任/触发仍需要独立宿主证据。hooks.state是状态表，不作为事件。文件版本哈希每次读取仅计算一次，JSON借用声明原文，不复制脚本或命令正文进清单/缓存。

配置响应hookRegistry独立提供授权项目的原生注册观察：启用、信任、处理类型、声明文件版本及注册哈希，不表示运行或注入。按校验后的结构、来源身份和文件哈希绑定普通文件键、插件独立JSON及内联声明键。原生版本仅用于诊断，不用版本白名单决定是否接入；当前CODEX_HOME必须属于所选来源，项目必须显式授权，不用来源目录冒充工作目录。Node先有界预检本地Hook声明及显式启用的插件；两者均无时不启动原生宿主。预检另有每文件10MiB/总32MiB预算。Node在两次一致的hooks/list之间对授权范围内普通文件流式计算哈希，Rust再次匹配当前清单版本；进入查询时超过60秒、原生失败、必需字段无效、重复身份、未知键形状或路径越界保持未知/partial。最多64个项目、512条注册、每文件10MiB/总32MiB哈希读取、64KiB缓冲和5秒查询取消期限；文件系统阻塞不承诺硬实时。完整注册观察不持久缓存，不导出命令/任意报错文本；发现与用户决定保留复查所需的项目、注册哈希及文件版本。每次前台读取重新获取当前观察，时间戳不改变相同证据的配置版本。CLI 文本与 JSON 共享结果。Unix已开放下述静态命令子集的hookTarget检查；Windows命令及运行证据仍待补。

插件注册包含明确pluginId，键中的身份、包内相对路径、事件及位置必须一致；相对路径不得含空分量、点分量或反斜线，并须与授权普通文件的路径后缀精确匹配。只读取原生列出的独立JSON或plugin.json内联声明；内联单对象及对象数组按原生hooks[n]位置绑定，测量真实JSON指针处的处理声明。清单阶段每个物理文件只读一次，不猜测缓存布局或遍历插件目录。清单只纳入注册涉及的声明与授权项目，不把插件配置视为全局生效；安全文件元数据及最近项目范围可保留，注册身份/状态仍只在视图中。注册缺失但来源文件未成功读取时，保留最近声明为stale；明确文件不存在或成功读取后声明消失才转历史。文件版本变化须重新绑定；未知形状保持未知。CLI 文本显示插件身份，JSON使用生成契约。

hookTarget仅在当前文件版本匹配、原生明确启用且trusted/managed时检查；显式禁用可复查通过，缺注册/未信任/已变更保持未知。Unix使用锁定shlex2.0.1解析，不执行命令。插件命令采用Codex返回的已展开结果，包括原生插件目录/数据目录；正文仅经私有内核消息临时传入，单条上限16KiB、完整捕获上限768KiB，不进入公开DTO或缓存。结果须匹配项目、插件身份、注册哈希及当前文件的有效command声明；缺结果、版本变化或残留动态表达式保持未知，Wombat不自行展开变量。子集为python/python2/python3/python3.N（独立-B/-E/-I/-s/-S/-u/-q或--）、sh/bash/dash/zsh（独立-e/-u/-x/-v/-n或--，脚本名须含/）、无选项node/nodejs或--，以及带./、../或/的.sh/.py/.js/.mjs/.cjs直接路径。内联执行、模块执行、未知选项、环境赋值、管道/替换/通配/控制字符均未知。相对基址来自原生注册工作目录；按文件系统顺序检查授权边界及软链，不以权限错误或目录类型当缺失。Node缺入口时还排除.js/.json/.node扩展候选；目录或候选存在时保持解析未知。这里只断言静态引用，不证明解释器身份、脚本有效或运行失败。

分析仅重读所需声明文件，每文件10MiB/单轮32MiB，同文件只解析一次；只保留静态路径及最多512条注册的稀疏检查，不建清单×项目矩阵。建议绑定项目、原生键/哈希和文件版本；历史按原来命中的项目复查，其他项目的新问题独立待处理，缺原授权/注册不通过。交接工作目录从实际命中的授权项目选择；声明字数未知不阻止版本已核验的脚本引用交接。原生确认的父子项目继承只补充当前视图，不写配置缓存；原生暂时不可读时，当前授权对象的项目历史和决定仍保留，复查结果为未知。

Skill YAML 校验必填字段类型、名称及已核验目录匹配，选填 license/allowed-tools/compatibility 字符串和 metadata 字符串映射；未知宿主字段不直接报错。名称按[官方参考校验器](https://github.com/agentskills/agentskills/blob/main/skills-ref/src/skills_ref/validator.py)去两端空白、NFKC、Unicode小写/字母数字/连字符及64码点检查，compatibility≤500码点。description 长度只由字符串字段产生；头部64 KiB、事件10,000、深度32，无别名/include，重复键拒绝。超限或未支持标签不报格式错误。最近匹配来源/项目的原生Skill目录包含对象时显示最近可用；目录缺失不宣称禁用。使用方法 v4 按规范化调用身份计数明确读取、MCP 尝试，以及确定命令参数与适配器核验目标一致的原生终态读取。失败尝试仍计数；用户 Shell 命令、已拒绝的原生读取、目录发现和仅供显示的读取标签不计数。身份、目标和日期缺口与已知正向记录分开保留。日期只筛选历史活动，不改变最近目录状态；关联轮次用量不是配置独占费用。

MCP 解析读取原生 mcp_tool_call_begin/end 和 McpToolCall 持久项，结果缺失保留未知。调用身份按任务/轮次隔离；明确分叉的同一上游轮次与调用归回有记录的祖先，追加和重启保留相同结果。资源请求只保留服务与操作类型，不保存 URI、参数或返回正文。保留名没有明确对应请求时操作类型未知；更多边界见[决定](../decisions/implemented/architecture/2026-10-04-mcp-runtime-evidence.md)。

关联用量按可靠关联轮次中匹配范围的计量去重；不分摊配置独占成本，各行可能重叠，汇总用并集。没有关联保留缺失，完整轮次量另查原用量版本。每次证据查询仍遍历安全事实，增量关联索引、持久复合版本及协作取消未实现；取消断开传输并隔离迟到结果，扫描受资源预算约束。

currentItems 不计明确缺失的路径，清单仍公开缺失行；显式读取以准确路径证据关联，不因 cwd 为授权根的子目录丢弃。

派生 config-v2 原子 JSON 只存安全元数据，每范围最多20,000项，旧缓存读取16 MiB。不存配置正文、环境值、命令参数或工具输出；不联网扫描，不执行 MCP。它可重建，与不可重建的用户处理记录分开。

`Request.review` 通过宿主私有通道按需增加原生 Skill 观察。Rust 生成 `contextFootprint` 和分页前的 `extensionReview` 汇总。版本化参考渲染器使用现有参考分词器计量名称、原生完整描述及文件绝对定位；尚未观察别名、提示预算截断或动态选择。`visibility` 保持未确认，`confirmedVisibleTokens` 保持 null。停用、仅磁盘发现和不可用对象保持区分。原生清单中位于文件读取授权范围外的条目仅保留元数据，不读取正文（`nativeMetadataOnly`）。原生状态与元数据绑定读取视图和宿主版本，不从磁盘缓存复用。MCP 和 Hook 缺少可靠载荷时不计量上下文。部分使用次数是下界，与完整次数分开呈现；使用归并与账本计算保持不变。

## 优化与偏好契约

Rust 的 `optimize_dto.rs` 生成[请求](../schemas/optimize-request-v2.schema.json)与[响应 v7](../schemas/optimize-response-v7.schema.json)。支持 list/detail/checks/capabilities、显式 keep/not_applicable 和 recheck，以及固定轮次 activity。仅命名偏差的 Skill 发现标为 `advisory`，归入 `review`，不证明运行故障；原生项目适用范围与文件的项目／全局归属分别保留。目录仅列十项已实现的静态规则；`partial` 来自实际范围或检查证据缺口，即使没有命中也保留缺口。

项目检查与复查会把目录别名解析为同一规范身份，但不会增加授权；外层项目报告回显提交的范围，内部检查使用规范身份。两条入口使用同样的新鲜宿主 Skill 发现信息，保留观测到的项目关联。本地资源的问题身份绑定来源对象和所需资源目标，不依赖可变内容哈希、错误状态或行号。

检查与复查只读，不创建自动观察或采用历史。基线 ID 绑定传回的原始发现、来源和项目范围及规则。复查校验该范围，检查当前配置并比较独立检查。范围或规则变化时拒绝；对象缺失或规则不适用表示证据不足，不能当作修复。部分覆盖的轮次检查可以通过 `observedSubset` 单独说明可分析命令子集的未命中结果，不替代整个轮次的证据不足判断。检查结论与比较状态分别表达。

只有显式用户决定写入独立用户存储。现行决定存储格式保持为 4，以保留有效决定与身份；只读检查不追加事件或修改文件。不迁移未知版本，不删除原数据。索引重建不能改变用户决定。历史处理展示、自动记录、采纳对比、重新显示和发送任务已移除。

`preferences` 保持版本 1，仅保存或读取 `zh`/`en`。本地语言对象只提供读取、切换和稳定翻译函数，没有订阅、修订号或冻结快照。计数、计价与原文不参与翻译。

## 账户契约 v1

`core/src/account_dto.rs`生成[请求](../schemas/account-request-v1.schema.json)与[响应](../schemas/account-response-v1.schema.json)，CLI共用read/refresh。账户身份、额度、活动各有状态和读取时间；无凭据字段。Node读取前后身份，Rust检查账户归属并归一化；失败保留旧时间且标stale，换账户清除先前事实。

windows保留真实百分比、周期和重置；buckets分别返回原生credits、individualLimit和明确限制状态，不从百分比推定普通使用权限。余额及消费限额为校验后的原始小数字符串，来源未提供单位时不补货币或Token单位。异常字段标partial并保留合法同级字段，不使其他窗口变零。每次最多128桶；超限明确partial，不回退旧单桶。

resetCredits保留来源availableCount，不以明细数量替代；credits=null表示未提供，空列表表示返回空列表。明细最多128条，超限标detailsTruncated和partial。权益状态、授予/到期时间仅展示来源事实，没有兑换或重置接口。已过重置时间不代表恢复，旧数据不作为新鲜权限。windows/buckets的model来自normalModelSlug，只是额度别名的展示元数据，不表示任务使用该桶。modelRestriction只保留身份一致、已支持原生通知中的blocked_model_slug及reset_at；不保存通知文案、链接或动作。




## Skill 契约

只交付 `wombat:wombat` 插件主 Skill。Rust 生成发现 DTO；Node 使用有界只读 app-server stdio 查询原生 Skills、Hooks 和账户，不提供任务发送或队列方法。`wombat-runtime.json` 绑定资源哈希与能力；缺失、修改或不支持分别报告。没有独立 Skill 安装契约或采集插件包。

setup 版本 1 默认只检查插件发现与运行时。`includeHooks` 默认为 false；显式请求后独立查询 Hook 注册。可选探测失败不掩盖已完成的发现和兼容性结果。CLI 对应 `setup --hooks`，具体步骤见 [CLI 指南](../guides/cli.md)。

对话请求和结果版本 7 新增 `source_search`。其 `sourceHits` 返回原文片段、UTF-8 命中范围、原始 JSONL 字节范围及该行摘要。各条命中通过 `fileIndex` 引用共享的 `sourceFiles` 条目。续页时，`sourceScope` 绑定日志清单。直搜设置 `contextMessages` 后，各命中的 `context` 按来源顺序引用共享的 `sourceNeighbors`。每条相邻消息保留自己的原始记录范围和摘要。索引查询 `search` 对每个命中的事件返回一条结果。搜索接受单个 `query` 或 `terms` 数组。续页时保留词及其顺序。用 `readCursor` 回读命中，片段带回原文上下文和 `matchRange`。该范围按返回文本内的 UTF-8 字节计算。

用 `complete` 采用扩展扫描预算。搜索结果保留原 `roots` 和 `selection`，扫描未完成时通过 `nextSearch` 返回完整续查请求。读取时可用 `contextMessages` 请求可见消息窗口。各位置按事件 ID 引用共享的 `neighbors`。

每个读取目标通过从零开始的 `targetIndices` 映射到一个原文位置或一个 `readIssues` 条目。相同文本可以共享一份正文，各个 `alsoAt` 位置和游标分别保留。推迟条目对应原读取目标。`history read` 封装版本 3 通过 `retryRead` 提供推迟目标的读取请求，通过 `nextLocations` 提供下一页位置请求。

在活动、定位或搜索扫描中设置 `fixedScope`，获取 `sourceScope`。扫描游标带有相同的范围。回读导出证据时，传入这个 `sourceScope`，并保留相同的来源根目录和筛选条件。范围变化时，核心返回 `SOURCE_CHANGED`，调用方应从第一页重新开始导出。

精确操作筛选绑定单个聊天和轮次，使用规范调用／条目别名，并保留原日期、来源和项目限制。工具请求与结果都有原文定位，读取工具文本需显式选择。正文保留规则及读取上限见 [核心参考](../reference/core.md)。
