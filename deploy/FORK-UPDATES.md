# la dmit 定制分支更新

本 fork 的部署分支为 `cpr-custom`，更新源固定为 `FlodQWQ/codex-proxy-rs`。
上游同步不会自动部署到 VPS。

## 构建与检查

`Fork Build` 对定制分支构建 Linux amd64 / Debian 12 兼容包，运行前端检查、
Host 更新器测试、OpenAI Provider 测试（含调度、额度和打票）、手动脚本校验测试和 Go 探针测试。成功后发布到本 fork 的 GitHub Releases。
版本为 `上游版本-fork.N`，N 使用工作流运行编号。发布仅来自 `cpr-custom`，标为
Pre-release，不覆盖 GitHub Latest，也不发布或覆盖 Docker 镜像。

程序只从本 fork 检查更高的 `fork.N` 版本，同一大版本内可随上游提升小版本。
不会切换到官方、alpha/beta/rc/exp 通道，不接受降级或跨大版本更新。
检查失败与无更新分开报告。网页可以显示新版本和发布链接，但不支持直接安装定制包，
防止独立 `codex-ticket-probe` 与主程序错配。

旧的 `fork.<提交号>` 构建不支持该检查通道，第一次也必须手动更新。
程序、脚本和发布机制的变更只有在安装新包后才对运行中的服务生效。

## 手动安装

从本 fork 对应的 `cpr-custom` Release 下载这三个附件并上传到 VPS 同一目录：

- `codex-proxy-rs-linux-amd64.tar.gz`
- `SHA256SUMS`
- `update-cpr.sh`

不要使用 Actions 的旧产物、上游 Release 或其他来源的脚本/校验和。
SHA-256 检测传输损坏，不替代对下载来源的信任。

先校验（不会修改部署）：

```bash
bash update-cpr.sh --check codex-proxy-rs-linux-amd64.tar.gz SHA256SUMS
```

首次升级先下载同分支的 `migrate-cpr-config.py`，检查并迁移旧配置（不会重启服务）：

```bash
sudo python3 migrate-cpr-config.py --check
sudo python3 migrate-cpr-config.py --apply
```

脚本默认读取 `/opt/codex-proxy-rs/deploy/config.yaml`，也可在命令末尾指定配置路径。
只新增同值的 `openai.residency`，保留旧字段以兼容回滚，不改其他配置或注释。
已有新值相同则不重复写入；新旧值冲突、重复键、YAML 别名或不支持的结构会拒绝迁移。
写入前在原目录创建权限为 `0600` 的完整备份，通过同目录原子替换保留配置属主和权限。
备份包含凭据，应妥善保护。迁移脚本不自动运行；与升级脚本不要并发执行。

确认后，在维护窗口显式执行（使用同分支最新 `update-cpr.sh`，旧 v3.12.1-fork.11 附件会拒绝保留的旧字段）：

```bash
sudo bash update-cpr.sh --apply codex-proxy-rs-linux-amd64.tar.gz SHA256SUMS
```

脚本针对当前 la dmit：`/opt/codex-proxy-rs`、`cpr.service`、Linux x86_64。
要求 Python 3.11+、python3-yaml、curl、tar、flock 和 systemd；不自动安装依赖。
它只使用本地附件，不下载、不拉取分支、不自动升级系统。
检查通过后会短暂停止服务，替换主程序、探针、`web`、`VERSION` 和 `REVISION`，
并写入专用 systemd drop-in 固定 `CPR_UPDATE_REPOSITORY=FlodQWQ/codex-proxy-rs`。
启动后验证本机 `/healthz`，失败尝试恢复旧文件及 drop-in；退出后仍应人工确认业务请求。

旧文件保留在 `/opt/codex-proxy-rs-backups/update-*`，解包及失败文件保留在
`/opt/codex-proxy-rs/.update-stage-*`，不自动清理。脚本不会覆盖 `deploy/config.yaml`、
`.runtime`、账号数据或数据库，也不执行数据库恢复。
更新前请自行备份数据库；程序启动时仍可能执行其自带数据库迁移，文件回滚不等于数据库回滚。
从 v3.11.0 到当前 v3.12.1 没有新增 SQL 迁移。

## 配置兼容

- 如显式配置了 `host.system_update.update_repository`，需改为 `FlodQWQ/codex-proxy-rs`。
- 如有 `openai.wire_profile.residency`，运行上述迁移脚本复制到 `openai.residency`；新旧值不一致时升级脚本在停机前拒绝继续。
- 旧 `openai.wire_profile` / `xai.wire_profile` 不再读取；数据库中已有 OpenAI 身份保留，
  YAML 中定制的 xAI 身份应在管理端重新设置。
- `/healthz` 从 YAML 的 `host.listen.port` 推导；如 systemd 环境变量另行覆盖监听地址/端口，
  应先统一配置再使用此脚本。
