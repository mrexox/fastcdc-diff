import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'ava'

import { diff, apply, diffUsingSourceSignature, writeBinarySignature } from '../index.js'

test('correctly applies generated diff', (t) => {
  const diffPath = path.join(os.tmpdir(), 'a-b.diff')
  const resultPath = path.join(os.tmpdir(), 'b.result')

  diff('__test__/A.bin', '__test__/B.bin', diffPath, { minSize: 64, avgSize: 256, maxSize: 1024 })
  apply(diffPath, '__test__/A.bin', resultPath)

  const b = fs.readFileSync('__test__/B.bin')
  const bRes = fs.readFileSync(resultPath)

  t.is(Buffer.compare(b, bRes), 0)
})

test('calculates the same diff with file and signature', (t) => {
  const diffPath = path.join(os.tmpdir(), 'a-b.diff')
  const sigDiffPath=  path.join(os.tmpdir(), 'a.sig-b.diff')

  const sigPath = path.join(os.tmpdir(), 'a.sig')
  writeBinarySignature('__test__/A.bin', sigPath)
  diff('__test__/A.bin', '__test__/B.bin', diffPath)
  diffUsingSourceSignature(sigPath, '__test__/B.bin', sigDiffPath)

  t.is(Buffer.compare(fs.readFileSync(diffPath), fs.readFileSync(sigDiffPath)), 0)
})
