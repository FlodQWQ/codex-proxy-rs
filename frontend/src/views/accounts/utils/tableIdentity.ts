interface TableAccountIdentity {
  name?: string | null
  email?: string | null
  accountId?: string | null
  id: string
  notes?: string | null
}

export function accountTableName(account: TableAccountIdentity): string {
  return account.name?.trim() || account.email?.trim() || account.accountId || account.id
}

export function accountTableNotes(account: TableAccountIdentity): string {
  // 迁移元数据留在编辑备注中，列表只显示用户可读的内容，并去掉与标题重复的首行。
  const lines = (account.notes || '').split(/\r?\n/).filter(line => !/^\[sub2api:la-ultra:\d+\]/.test(line))
  if (lines[0]?.trim() === accountTableName(account))
    lines.shift()
  return lines.join('\n').trim()
}
