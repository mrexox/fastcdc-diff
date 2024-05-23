import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'ava'

import { diff, apply } from '../index.js'

test('correctly applies generated diff', (t) => {
  const diffPath = path.join(os.tmpdir(), 'a-b.diff')
  const resultPath = path.join(os.tmpdir(), 'b.result')

  diff('__test__/A.bin', '__test__/B.bin', diffPath)
  apply(diffPath, '__test__/A.bin', resultPath)

  const b = fs.readFileSync('__test__/B.bin')
  const bRes = fs.readFileSync(resultPath)

  t.is(Buffer.compare(b, bRes), 0)
})
