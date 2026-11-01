# 开发与交付约定

中文 | [English](workflow.en.md)

职责见[架构](architecture.md)，字段见[契约](contracts.md)，规则见 [AGENTS.md](../../AGENTS.md)。

## 代码规范

- 新能力通过 CLI 和生成的无 TTY 接口交付；修改公共 DTO 时检查所有消费者及取消、错误和状态语义。
- 配置、文件、进程及网络入口校验不可信数据；同进程已类型化值不重复校验。TypeScript 保持 strict；新增 any 或断言须说明无法收窄的原因，禁止双重断言绕过校验。
- 封闭联合按判别字段穷尽处理；开放来源值有未知分支。所属入口一次解析默认值，尽早报告可确认的配置错误。
- 操作本身执行授权及版本检查，不只靠页面禁用或包装过滤；验证直接及其他入口不能绕过。
- 每次异步操作由一个控制器或事务管理；额外状态须有独立职责。完成、失败与取消均结算；有界等待子任务退出，迟到结果不能更新失效视图。
- 持久提交成功才发布状态或通知；失败保留已提交事实。缓存与展示派生自同一权威结果。
- 完整输出或保留值可知时限制资源，包含信封、元数据及多字节编码；覆盖最小值、精确边界和单块超限。
- catch 只包围预期失败操作；说明忽略错误的原因，保留可观察失败。回调异常不能破坏无关请求或清理。
- 注释写调用者所需的行为、失败、时机及所有权；理由链接决策，不复述代码或审阅。更新所属说明，局部改动不建决策。


## 验证

`corepack pnpm repo:check` 不依赖 dist，CI 将其与平台构建独立运行。脚本遵循 [scripts/AGENTS.md](../../scripts/AGENTS.md)，范围见 [wombat-verify](../../.agents/skills/wombat-verify/SKILL.md)。审阅及行为测试验证语义，静态检查不能全覆盖。

```sh
corepack pnpm build
corepack pnpm typecheck
corepack pnpm contracts:check
corepack pnpm test
~/.cargo/bin/cargo fmt --manifest-path core/Cargo.toml -- --check
~/.cargo/bin/cargo clippy --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings
```

按改动选命令，完整链路执行全部。跨语言测试先构建 dist；不重复未受影响的通过检查。用真实装配入口验证产品行为，手工 mock 不能证明交付；可替换外部服务及非确定输入。计量用[独立真值](adapters.md)，不只以旧输出为依据。

分别验证 CLI、真实内核和安装资产；通过组装入口检查取消、失败与恢复。性能用固定语料和 release，记录缓存、启动、耗时、峰值内存及结果一致性；基准见 package.json 的 benchmark 脚本。

依赖变化运行 `corepack pnpm licenses:generate` 和 `corepack pnpm licenses:check`。按[发行 Skill](../../.agents/skills/wombat-release/SKILL.md)验证目标平台及干净安装。只报告实际验证，构建不替代产品或平台验收。

CI 执行、缓存与时间分析见 [CI](ci.md)。

### 失败排查

- `test:repo`、`test:prebuild` 不依赖构建产物；CI 在 Linux/macOS/Windows 各运行一次，与五平台原生构建并行。本地发行在 Rust 编译前检查源码，不替代产品或安装验收。
- 时间计算保留来源精度；以实例身份判断进程归属，PID/端口可能复用。探测与外层截止时间竞速时用稳定失败代码；未确认不代表清理成功。
- 区分启动、数据就绪及验收。首次读取可返回扫描预览；最终统计须读取已完成数据并绑定同一版本，不以任意等待证明就绪。
- 停止重试，保留阶段、日志、源码版本及环境，追查根因。陌生行为先查上游文档；重跑、调参或延长超时不能代替分析。
- 检查共享假设的调用方、平台、构建前置及 CI/发行流程，批量修复并补回归后验证。
- 先复现整类问题，再统一检查改动，之后才运行昂贵的平台或发行任务；只复用仍有效的通过证据。
