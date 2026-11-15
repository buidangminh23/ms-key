import { test } from 'node:test';
import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { withProgress } from './progress.mjs';

test('redirected and reduced-motion output contains no terminal controls', async () => {
  for (const [isTTY, env] of [[false, {}], [true, { MSKEY_REDUCE_MOTION: '1' }], [true, { TERM: 'dumb' }]]) {
    let text = '';
    await withProgress('Downloading', async (update) => { update({ received: 5, total: 10 }); }, { isTTY, write(value) { text += value; } }, env);
    assert.doesNotMatch(text, /\x1b|\r/);
    assert.match(text, /Download verified/);
  }
});

test('failed download clears animation and does not report success', async () => {
  let text = '';
  const output = { isTTY: true, columns: 80, write(value) { text += value; } };
  await assert.rejects(withProgress('Downloading', async (update) => {
    update({ received: 3, total: 10 });
    await delay(110);
    throw new Error('checksum mismatch');
  }, output, { NO_COLOR: '1' }), /checksum mismatch/);
  assert.match(text, /30%/);
  assert.match(text, /Download failed/);
  assert.doesNotMatch(text, /Download verified/);
  const finished = text;
  await delay(110);
  assert.equal(text, finished);
});
