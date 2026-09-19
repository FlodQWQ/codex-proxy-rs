import assert from 'node:assert/strict'
import { accountTableName, accountTableNotes } from '../src/views/accounts/utils/tableIdentity.ts'

assert.equal(accountTableName({ id: '1', name: ' plus ', email: 'mail@example.invalid' }), 'plus')
assert.equal(accountTableName({ id: '1', name: '', email: 'mail@example.invalid' }), 'mail@example.invalid')
assert.equal(accountTableName({ id: '1' }), '1')
assert.equal(accountTableNotes({ id: '1', name: 'plus', notes: 'plus\n[sub2api:la-ultra:35] source_status=active\nUser note' }), 'User note')
assert.equal(accountTableNotes({ id: '1', name: 'plus', notes: 'Custom note\nSecond line' }), 'Custom note\nSecond line')
assert.equal(accountTableNotes({ id: '1', name: 'plus', notes: 'plus' }), '')
assert.equal(accountTableNotes({ id: '1', name: 'plus', notes: '[manual note]' }), '[manual note]')
console.log('Account table identity checks passed (7 assertions)')
