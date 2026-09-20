#!/usr/bin/env python3
"""显式迁移 residency；不启动服务，不打印配置内容。"""
import argparse
import os
from pathlib import Path
import stat
import tempfile
import yaml


def migrate(text):
    # 拒绝重复键和别名，避免改到 YAML 合并或共享节点所指的其他位置。
    tokens = list(yaml.scan(text))
    if any(isinstance(token, (yaml.tokens.AliasToken, yaml.tokens.AnchorToken)) for token in tokens):
        raise ValueError('不支持含 YAML 锚点或别名的配置，请人工确认')
    root = yaml.compose(text)

    def fields(node):
        if not isinstance(node, yaml.MappingNode) or node.flow_style:
            raise ValueError('配置必须使用块式映射')
        result = {}
        for key, value in node.value:
            if key.value in result or key.value == '<<':
                raise ValueError('配置包含重复键或合并键')
            result[key.value] = (key, value)
            if isinstance(value, yaml.MappingNode):
                fields(value)
        return result

    top = fields(root)
    if 'openai' not in top:
        return text
    section = fields(top['openai'][1])
    if 'wire_profile' not in section:
        return text
    legacy = fields(section['wire_profile'][1])
    if 'residency' not in legacy:
        return text
    old = legacy['residency'][1]
    if not isinstance(old, yaml.ScalarNode):
        raise ValueError('residency 必须是标量')
    value = yaml.safe_load(text[old.start_mark.index:old.end_mark.index])
    if value is None:
        return text
    if not isinstance(value, str):
        raise ValueError('residency 必须是字符串')
    if 'residency' in section:
        current = section['residency'][1]
        if current.tag != old.tag or current.value != old.value:
            raise ValueError('新旧 residency 冲突，未修改配置')
        return text
    key = section['wire_profile'][0]
    newline = '\r\n' if '\r\n' in text else '\n'
    import json
    insertion = 'residency: ' + json.dumps(value, ensure_ascii=False) + newline + ' ' * key.start_mark.column
    updated = text[:key.start_mark.index] + insertion + text[key.start_mark.index:]
    expected = yaml.safe_load(text)
    expected['openai']['residency'] = value
    if yaml.safe_load(updated) != expected:
        raise ValueError('迁移后语义校验失败')
    return updated


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check', action='store_true')
    mode.add_argument('--apply', action='store_true')
    parser.add_argument('config', nargs='?', default='/opt/codex-proxy-rs/deploy/config.yaml')
    args = parser.parse_args()
    path = Path(args.config).absolute()
    if path.is_symlink() or not path.is_file():
        raise ValueError('配置必须是普通文件，不能是符号链接')
    original = path.read_bytes()
    info = path.stat()
    updated = migrate(original.decode('utf-8')).encode('utf-8')
    if updated == original:
        print('无需迁移：已兼容或没有旧 residency 配置。')
        return
    if args.check:
        print('需要迁移 openai.residency；仅检查，未写入。')
        return
    # 旧项保留，既不丢注释，也允许旧程序在文件回滚后继续使用原约束。
    backup_fd, backup = tempfile.mkstemp(prefix=path.name + '.before-residency-', dir=path.parent)
    with os.fdopen(backup_fd, 'wb') as stream:
        stream.write(original)
        stream.flush()
        os.fsync(stream.fileno())
    fd, temporary = tempfile.mkstemp(prefix=path.name + '.migration-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(updated)
            os.fchown(stream.fileno(), info.st_uid, info.st_gid)
            os.fchmod(stream.fileno(), stat.S_IMODE(info.st_mode))
            stream.flush()
            os.fsync(stream.fileno())
        if path.is_symlink() or path.read_bytes() != original:
            raise ValueError('配置在迁移期间改变，拒绝覆盖')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    print('配置迁移完成，未重启服务；原始备份：' + backup)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, yaml.YAMLError):
        raise SystemExit('迁移失败：配置结构不支持、新旧值冲突或文件操作失败；未输出敏感内容。')
