#!/usr/bin/env bash
# 只在操作者显式传入 --apply 时替换 la dmit 上的文件。
set -Eeuo pipefail
umask 022

mode=${1:---help}
if [[ $# != 3 || ! "$mode" =~ ^--(check|apply)$ ]]; then
  echo '用法：bash update-cpr.sh --check|--apply 压缩包路径 SHA256SUMS路径'
  echo '固定目标：/opt/codex-proxy-rs，systemd 单元 cpr.service。默认不更新。'
  exit 2
fi
archive=$(realpath -- "$2")
checksums=$(realpath -- "$3")
app=/opt/codex-proxy-rs
service=cpr.service
dropin=/etc/systemd/system/cpr.service.d/90-fork-update-source.conf

# 不信任归档成员、文件名或校验清单中的路径；只接受普通文件和目录。
metadata=$(python3 - "$archive" "$checksums" <<'PY'
import hashlib, pathlib, re, sys, tarfile
archive, sums = map(pathlib.Path, sys.argv[1:])
entries = [line.split() for line in sums.read_text().splitlines() if line.strip()]
matches = [parts[0] for parts in entries if len(parts) == 2 and parts[1].lstrip('*') == archive.name]
if len(matches) != 1 or not re.fullmatch(r'[0-9a-fA-F]{64}', matches[0]):
    raise SystemExit('SHA256SUMS 必须恰好包含一个匹配的归档文件名')
with archive.open('rb') as stream:
    actual = hashlib.file_digest(stream, 'sha256').hexdigest()
if actual != matches[0].lower():
    raise SystemExit('SHA256 校验失败，未更改部署')
with tarfile.open(archive, 'r:gz') as package:
    seen = set()
    members = {}
    total = 0
    for member in package:
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or '..' in path.parts or '\\' in member.name:
            raise SystemExit('归档含不安全路径')
        if not (member.isfile() or member.isdir()) or member.mode & 0o7000:
            raise SystemExit('归档含链接、特殊文件或特殊权限')
        name = str(path)
        if name in seen:
            raise SystemExit('归档含重复路径')
        seen.add(name)
        members[name] = member
        total += member.size
        if total > 512 * 1024 * 1024 or len(seen) > 20000:
            raise SystemExit('归档超过解包限制')
    for name in ('codex-proxy-rs', 'codex-ticket-probe', 'web/dist/index.html', 'VERSION', 'REVISION'):
        if name not in members or not members[name].isfile():
            raise SystemExit('归档缺少必要文件：' + name)
    version = package.extractfile(members['VERSION']).read(200).decode().strip()
    revision = package.extractfile(members['REVISION']).read(200).decode().strip()
    if not re.fullmatch(r'3\.\d+\.\d+-fork\.[1-9]\d*', version):
        raise SystemExit('只接受 v3 fork.N 产物')
    if not re.fullmatch(r'[0-9a-f]{40}', revision):
        raise SystemExit('提交号无效')
    for name in ('codex-proxy-rs', 'codex-ticket-probe'):
        header = package.extractfile(members[name]).read(20)
        if header[:6] != b'\x7fELF\x02\x01' or header[18:20] != b'\x3e\x00':
            raise SystemExit('仅支持 Linux x86_64 ELF：' + name)
    print(version, revision)
PY
)
read -r version revision <<< "$metadata"
printf '校验通过：%s\n提交：%s\n目标：%s (%s)\n' "$version" "$revision" "$app" "$service"
if [[ "$mode" == --check ]]; then
  echo '仅校验，没有停止服务、写入部署或修改配置。'
  exit 0
fi

[[ $EUID == 0 ]] || { echo '--apply 必须以 root 执行' >&2; exit 1; }
[[ $(uname -m) == x86_64 && $(uname -s) == Linux ]] || exit 1
for command in systemctl curl flock tar python3; do command -v "$command" >/dev/null; done
[[ $(realpath -e "$app") == "$app" && ! -L "$app" ]] || exit 1
[[ $(systemctl show "$service" -p WorkingDirectory --value) == "$app" ]] || exit 1
systemctl is-active --quiet "$service"
exec 9>/run/lock/cpr-manual-update.lock
flock -n 9 || { echo '另一个更新正在执行' >&2; exit 1; }

# 不改写配置文件或真实凭据；遇到必须迁移的旧项时，在停机前退出。
health_url=$(python3 - "$app/deploy/config.yaml" <<'PY'
import sys, yaml
with open(sys.argv[1]) as stream:
    config = yaml.safe_load(stream)
host = config['host']
source = host.get('system_update', {}).get('update_repository')
if source not in (None, 'FlodQWQ/codex-proxy-rs'):
    raise SystemExit('请先将 host.system_update.update_repository 改为 FlodQWQ/codex-proxy-rs')
if config.get('openai', {}).get('wire_profile', {}).get('residency') is not None:
    raise SystemExit('请先将 openai.wire_profile.residency 迁移到 openai.residency')
port = host['listen']['port']
if not isinstance(port, int) or not 1 <= port <= 65535:
    raise SystemExit('监听端口无效')
print(f'http://127.0.0.1:{port}/healthz')
PY
)
curl --noproxy '*' --fail --silent --show-error --max-time 5 "$health_url" >/dev/null
[[ ! -L "${dropin%/*}" && ! -L "$dropin" ]] || exit 1
targets=(codex-proxy-rs codex-ticket-probe web VERSION REVISION)
for item in "${targets[@]}"; do [[ ! -L "$app/$item" ]] || exit 1; done
if [[ -f "$app/VERSION" ]]; then
  python3 - "$app/VERSION" "$version" <<'PY'
import pathlib, re, sys
pattern = r'(3)\.(\d+)\.(\d+)-fork\.([1-9]\d*)'
old = re.fullmatch(pattern, pathlib.Path(sys.argv[1]).read_text().strip())
new = re.fullmatch(pattern, sys.argv[2])
if old and tuple(map(int, new.groups())) <= tuple(map(int, old.groups())):
    raise SystemExit('拒绝降级或重复安装')
PY
fi

backup_root=/opt/codex-proxy-rs-backups
[[ ! -L "$backup_root" ]] || exit 1
install -d -m700 "$backup_root"
backup=$(mktemp -d "$backup_root/update-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX")
stage=$(mktemp -d "$app/.update-stage-XXXXXX")
tar --extract --gzip --file "$archive" --directory "$stage" --no-same-owner --no-same-permissions
chmod 755 "$stage/codex-proxy-rs" "$stage/codex-ticket-probe"
chmod -R a+rX "$stage/web"
had_dropin=false
if [[ -f "$dropin" ]]; then cp -a "$dropin" "$backup/update-source.conf"; had_dropin=true; fi
changed=()
rollback() {
  local result=$?
  [[ $result != 0 ]] || result=1
  trap - ERR INT TERM
  echo "更新未完成，恢复旧文件；备份：$backup" >&2
  systemctl stop "$service" || true
  for item in "${changed[@]}"; do
    if [[ -e "$app/$item" ]]; then mv "$app/$item" "$stage/failed-$item"; fi
    if [[ -e "$backup/$item" ]]; then mv "$backup/$item" "$app/$item"; fi
  done
  if $had_dropin; then
    cp -a "$backup/update-source.conf" "$dropin"
  elif [[ -f "$dropin" ]]; then
    mv "$dropin" "$backup/failed-update-source.conf"
  fi
  systemctl daemon-reload || true
  systemctl start "$service" || true
  echo '仅回滚程序文件；数据库未被脚本修改或恢复。请检查服务健康状态。' >&2
  exit "${result:-1}"
}
trap rollback ERR INT TERM
systemctl stop "$service"
for item in "${targets[@]}"; do
  if [[ -e "$app/$item" ]]; then mv "$app/$item" "$backup/$item"; fi
  changed+=("$item")
  mv "$stage/$item" "$app/$item"
done
install -d -m755 "${dropin%/*}"
printf '[Service]\nEnvironment=CPR_UPDATE_REPOSITORY=FlodQWQ/codex-proxy-rs\n' > "$dropin"
chmod 644 "$dropin"
systemctl daemon-reload
systemctl start "$service"
healthy=false
for attempt in {1..30}; do
  if systemctl is-active --quiet "$service" && curl --noproxy '*' --fail --silent --max-time 2 "$health_url" >/dev/null; then
    healthy=true
    break
  fi
  sleep 2
done
$healthy
trap - ERR INT TERM
printf '已安装 %s。健康检查通过；旧文件保留在 %s\n' "$version" "$backup"
echo '配置、.runtime 和数据库未被脚本覆盖。临时目录保留供检查：' "$stage"
