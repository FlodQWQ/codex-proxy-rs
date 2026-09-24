# 移除插件 Provider 扩展计划

## 目标与边界

Provider 集合固定为内置 `openai`、`xai`，采用插件接入前的静态注册方式。
插件继续提供中间件、模型路由、账号调度、请求与用量观察、客户端认证、管理页面和 CLI。
插件仍可在已有权限与业务约束下调用宿主模型、管理 OpenAI/xAI 账号、访问网络和私有状态。

用户确认插件功能尚未发版。本次直接删除 Provider 扩展，不增加旧包兼容层、弃用期、升级流程或专门的数据迁移。
既有数据库迁移遵守冻结规则；保留通用插件安装、配置和私有状态持久化。

## 工作分支与执行安排

- 主仓库：`/home/zyy/Codes/codex-proxy-rs`，分支 `main`
- 起点：`54b6be52 feat(proxies): fill manual location from outlet probe`
- 工作目录：`/home/zyy/Codes/codex-proxy-rs-plugin-provider-removal`
- 工作分支：`refactor/remove-plugin-providers`
- 实现代理：`gpt-6-sol`，推理强度 `max`（用户指定的 6-sol-max）
- 在独立 worktree 实现和验证，随后由主代理集成
- main 上的代理位置提交原地保留；删除变更完成后集成到其后，不重写或撤销该提交
- 不推送、不创建 PR、不发布版本

实施前 main（`54b6be52`）已包含插件功能；行为参照是插件接入前的 `60e0d4ee`。
只能按本次职责恢复静态 Provider 行为，不能整块还原文件而丢失之后的代理、UI 或其他修复。

## 实施前核实的依据

- `backend/apps/gateway/src/bootstrap.rs`：组合根先构造 OpenAI/xAI，再通过插件 Runtime 扩展注册表；此前直接使用静态注册表。
- `backend/crates/gateway-plugin/runtime/src/generation/prepare/instance.rs`：每实例只有一个 Provider 描述符，Provider 专属能力由 executor 及七项附属能力组成。
- `backend/crates/gateway-plugin/runtime/src/adapter/provider/`：承载插件 Provider 执行、目录、凭据、额度、计费、画像及续写适配。
- `backend/crates/gateway-core/src/engine/extensions.rs`、`backend/crates/gateway-admin/src/ports/provider_extensions.rs`：动态 Provider 注册索引；其中通用调用作用域须按实际消费者保留。
- `backend/crates/gateway-plugin/sdk/src/call/host.rs`：宿主模型回调的 ModelEventBatch 复用了 call/provider 下的 ExecutionEvent，属于保留功能依赖。
- `frontend/src/components/client-profile/ProviderRequestProfilesEditor.vue`：当前请求 Provider 列表并维护加载、失败、动态配置和孤立画像状态；固定平台后应简化。
- 独立示例仓库 `/home/zyy/Codes/codex-proxy-plugins` 的 workbench 同时声明 Provider 和其他扩展，需同步核对其依赖。

## 实施步骤

### 1. 固定 Provider 注册

- [x] Core 和 Admin 直接接入 OpenAI/xAI 静态注册表，覆盖常规启动和插件 CLI 路径
- [x] 删除 ProviderExtensionIndex、ProviderAdminExtensionIndex 及专属动态查找、冲突检查和注册寿命管理
- [x] 保留其他插件扩展的快照发布、在途请求保活、故障隔离和递归保护
- [x] 保留现有 Provider trait、账号分组、模型路由、重试及两个内置 Provider 的业务实现

### 2. 删除插件 Provider 合同与实现

- [x] 删除 contributes.provider 作者简写与 PluginBuilder.provider 注册入口
- [x] 删除八项 Provider 能力：executor、models、authentication、quota、request_profile、account_management、billing、maintenance
- [x] 删除对应 RPC 方法、Provider 描述、执行适配、目录、凭据、额度、计费、画像、续写和专属维护任务
- [x] 清理 Runtime 的 ProviderStorePorts 注入、Provider 续写排空、专属诊断和失去消费者的辅助代码
- [x] 将仍被宿主模型回调使用的事件、编码等合同放到合适的共享职责位置，保留其真实调用行为
- [x] 保留 models/accounts 等访问权限、host.model/host.auth 回调和 frontend_authentication；能力删除不能扩大为权限删除
- [x] 更新宿主兼容能力清单；保留通用插件合同版本检查，不为了未发布删除增加版本迁移机制
- [x] 删除后沿调用链重新检查抽象：清理空 trait/枚举/错误类型、只剩转发的 wrapper、无独立职责的端口与注册表
- [x] 合并同一 owner 内失去独立意义的碎片文件、单用途转换和重复状态，移除空文件及空模块；按职责与真实消费者判断，不单凭行数合并
- [x] 不以空集合、恒定返回值、占位实现或多余泛化类型维持已经撤掉的 Provider 扩展结构

### 3. 前端固定平台与接口清理

- [x] 全面盘点账号、用量、请求画像等页面中查询 Provider 名单的接口调用
- [x] Provider 选择直接使用统一的 OpenAI/xAI 固定集合，移除仅为发现平台存在的请求、加载、失败和重试状态
- [x] 删除插件 Provider 专用的 schema 表单、通用画像编辑和孤立 Provider 提示等无消费者 UI
- [x] 清理不再需要的 API wrapper、后端路由/用例/存储查询及文档；用调用关系确认无剩余用途
- [x] 对照接入插件前的行为，保留账号实际状态、模型目录、额度、版本/画像内容等真正的业务查询
- [x] 原生平台的账号级能力仍以实际账号事实为准，不能把账号认证差异简单硬编码成平台名单
- [x] 保持 `54b6be52` 代理位置功能及之后并发提交完整

### 4. 测试、文档与示例

- [x] 删除纯 Provider 扩展测试，重写把 Provider 当作通用插件载体的 fixture，使其继续验证保留能力
- [x] 按改动同步 SDK 文档、架构、API、插件使用说明和开发指引，在原章节修订
- [x] 独立示例若需修改，在该仓库另建 worktree，保留其现有改动；移除回显 Provider 并保留可用的其他教学扩展
- [x] 不修改 AGENTS.md，不自动追加发布说明，不运行发布动作

## 验证与验收

按当前仓库贡献指南使用 Rust 1.97.0、前端固定 pnpm 版本和专用测试环境。
实际执行命令和结果记录在下方，未运行或跳过项列为缺口。

1. Rustfmt、严格 Clippy、受影响的 SDK/Runtime/Core/Admin/API 测试及架构边界检查
2. 前端 `format:check` 和 `build`（包含类型检查），以及 `git diff --check`
3. 运行行为：OpenAI/xAI 仍可正确注册、选择与处理请求，插件无法注册第三个 Provider
4. 保留插件行为：管理/CLI 调用现有模型、账号回调、中间件、路由/调度、观察和客户端认证按变更范围验证
5. 浏览器验收：相关 Provider 控件只显示 OpenAI/xAI，切换正常；网络记录中没有为发现可选 Provider 发出的请求
6. 真实推理或服务依赖不可用时明确记录，不以构建或模拟数据替代真实集成结论
7. 完成后核对 main 仍包含 `54b6be52`，检查并发变更后再将本分支集成到 main；不覆盖其他工作区内容

## 执行记录

- 计划创建：已从 `54b6be52` 创建独立 worktree；主工作区干净，代理位置提交已在 main。
- 实现状态：固定原生 Provider 注册、删去插件 Provider 能力与适配、OAuth 多账号与延迟发布残留、前后端发现接口及动态表单；保留其他插件扩展与原生业务查询。
- Rust 静态检查：`cargo fmt --all --check`、共享 target 上的 `cargo check --workspace --all-targets`、严格 `cargo clippy --locked --workspace --all-targets -- -D warnings` 和 `git diff --check` 均通过；Clippy 使用 `RUST_MIN_STACK=16777216`、`CARGO_BUILD_JOBS=1`，日志 `/tmp/cpr-provider-removal-clippy-final.log`。
- Rust 测试：专用隔离 PostgreSQL/Redis 下运行 `cargo test --locked --workspace --all-targets --no-fail-fast`，日志 `/tmp/cpr-provider-removal-workspace-tests-no-fail-fast.log`；该轮因 Runtime 9 项、SDK 1 项、Store 2 项失败而退出 101，其他目标通过。修复后 Runtime `cargo test --locked -p gateway-plugin-runtime --test main -- --test-threads=1` 为 104 通过、0 失败、1 ignored（`/tmp/cpr-provider-removal-runtime-serial.log`）；SDK `--features io` 对失败用例复验 1/1 通过（`/tmp/cpr-provider-removal-sdk-targeted.log`），Store 两个失败用例分别 1/1 通过（`/tmp/cpr-provider-removal-store-migration-targeted.log`、`/tmp/cpr-provider-removal-store-plugin-account-targeted.log`）。最后的插件身份命名调整后，CLI 3/3 与管理模型流 1/1 复验通过（`/tmp/cpr-provider-removal-cli-renamed-targeted.log`、`/tmp/cpr-provider-removal-management-renamed-targeted.log`）。并行 Runtime 曾一次出现早期进程退出计数 2/3，原断言在隔离单测和串行 Runtime 套件均通过；资源竞争只是推测，未据此放宽断言。
- 前端：工作目录 `node_modules` 为复用同锁依赖的临时软链接，`pnpm run` 会尝试安装并拒绝软链接；直接运行同一依赖的 `eslint .` 通过（0 error、1 条既有 `PluginVersionCard.vue` 属性顺序 warning），`vue-tsc -b --pretty false` 与 Vite 生产构建通过。
- 隔离浏览器：模拟 API 下账号/用量筛选及 OpenAI/xAI 画像编辑正常；39 个请求中三个 Provider 名单发现端点均为 0，缺失 fixture 与浏览器错误均为 0。验收限桌面亮色与模拟数据。
- 示例：独立 worktree `/home/zyy/Codes/codex-proxy-plugins-provider-removal`（`refactor/remove-provider-example`）已提交 `d2e4204d3dbc0442fc9eeb24273e42260705d126`，移除 Provider 演示与假账号入口，保留 9 类扩展并改用现有 Key 和模型。SDK/CLI 固定到网关提交 `eb4477ceca1a55a1a0067bea619d589e10bdda29`，Cargo.lock 保留规范 Git 来源，没有本机路径补丁。固定后 `cargo +1.97.0 test --locked --offline --manifest-path examples/workbench/backend/Cargo.toml` 18/18 通过，严格 Clippy 和 Rustfmt 通过。前端使用原示例工作区已安装依赖，直接执行 ESLint、vue-tsc、Vite build 均通过，未另做 frozen-lockfile 安装；模拟页面验证了 Key/模型选择及示例请求。原示例 main 上的 README、pnpm 和 UI 依赖改动完整保留；`refactor/remove-provider-example` 已快进合入示例 `main`，这四个文件的本地改动仍未提交。
- 验证缺口：上述全量命令并非单次全绿，修复后按实际失败目标与命名调整范围复验。两个既有 ignored 分别为 Host 的 GitHub 在线发行查询、Runtime 的独立打包 observer 示例集成；后者需要两个示例包及隔离日志订阅器。未连接真实上游执行 OAuth 授权或推理，未验收窄屏和暗色状态。SDK 提交通过仅该命令生效的本地 Git URL 重写获取，未推送，未验证从远端获取新提交。

## 首次本地集成结果（折叠前）

- 网关实现提交：`eb4477ceca1a55a1a0067bea619d589e10bdda29`，已快进合入 `main`，直接位于 `54b6be5286bb9bbfcea60abf1c908e833c392d70` 之后。
- `feat(proxies): fill manual location from outlet probe` 提交保留，代理用例、API、探测与前端功能文件相对该提交没有改动；插件账号查询补齐了它需要的位置字段。
- 冻结迁移的 20 项校验通过，本次未改迁移文件。
- 独立 worktree 与分支保留供查看；未推送、未创建 PR、未发布版本。

## 提交整理

- Provider 删除、计划与验证记录、示例合入记录已并入 `feat(plugin): add the complete plugin development and management workflow (#256)`。
- 后续四笔依赖更新与两笔代理功能提交继续独立；`feat(proxies): fill manual location from outlet probe` 保持在 main 末尾。插件账号读取位置字段的修复归入 #209 的代理位置提交。
- 示例 SDK/CLI 引用同步到整理后的插件工作流提交，具体 revision 以示例工程的 `Cargo.toml` 和锁文件为准；原有 README、pnpm 和 UI 依赖的未提交改动保留。
- 整理前历史保存在 `backup/main-before-plugin-provider-squash-20260924`，上文旧提交号记录当时的实施与验证状态。
