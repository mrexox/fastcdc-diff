import fs from 'node:fs'
import test from 'ava'

import { diff, apply } from '../index.js'

test('correctly applies generated diff', (t) => {
  diff('__test__/A.bin', '__test__/B.bin', '/tmp/a-b.diff')
  apply('/tmp/a-b.diff', '__test__/A.bin', '/tmp/b.result')

  const b = fs.readFileSync('__test__/B.bin')
  const bRes = fs.readFileSync('/tmp/b.result')

  t.is(Buffer.compare(b, bRes), 0)
})
