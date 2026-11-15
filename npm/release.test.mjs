import { test } from 'node:test';
import assert from 'node:assert/strict';
import { download, fetchVerifiedAsset, parseChecksums, pickAsset, sha256, targetKey } from './release.mjs';

const SUMS = [
  `${'a'.repeat(64)}  MSKey-0.1.0-linux-amd64.tar.gz`,
  `${'b'.repeat(64)}  MSKey-0.1.0-macos-universal.zip`,
  `${'c'.repeat(64)}  MSKey-0.1.0-windows-x64-portable.zip`,
  `${'d'.repeat(64)}  MSKey-Setup-0.1.0-windows-x64.exe`,
  `${'e'.repeat(64)}  install-macos.sh`,
  `${'f'.repeat(64)}  mskey_0.1.0_amd64.deb`,
].join('\r\n');

test('parses SHA256SUMS with CRLF line endings', () => {
  const sums = parseChecksums(`${SUMS}\r\n`);
  assert.equal(sums.size, 6);
  assert.equal(sums.get('install-macos.sh'), 'e'.repeat(64));
});

test('rejects malformed checksum lines', () => {
  assert.throws(() => parseChecksums('not-a-hash  file'), /Malformed/);
});

test('maps each platform to exactly one release asset', () => {
  const sums = parseChecksums(SUMS);
  assert.equal(pickAsset(sums, targetKey('win32', 'x64', false)).name, 'MSKey-Setup-0.1.0-windows-x64.exe');
  assert.equal(pickAsset(sums, targetKey('darwin', 'arm64', false)).name, 'MSKey-0.1.0-macos-universal.zip');
  assert.equal(pickAsset(sums, targetKey('darwin', 'x64', false)).name, 'MSKey-0.1.0-macos-universal.zip');
  assert.equal(pickAsset(sums, targetKey('linux', 'x64', true)).name, 'mskey_0.1.0_amd64.deb');
  assert.equal(pickAsset(sums, targetKey('linux', 'x64', false)).name, 'MSKey-0.1.0-linux-amd64.tar.gz');
});

test('has no build for unsupported platforms', () => {
  assert.equal(targetKey('linux', 'arm64', true), null);
  assert.equal(targetKey('win32', 'arm64', false), null);
  assert.equal(targetKey('freebsd', 'x64', false), null);
});

test('verifies the downloaded asset against SHA256SUMS', async () => {
  const body = Buffer.from('installer bytes');
  const files = {
    SHA256SUMS: `${sha256(body)}  mskey_0.1.0_amd64.deb\n`,
    'mskey_0.1.0_amd64.deb': body,
  };
  const fetchImpl = async (url) => {
    const name = url.split('/').pop();
    return name in files
      ? new Response(files[name])
      : new Response('missing', { status: 404 });
  };
  const asset = await fetchVerifiedAsset('linux-x64-deb', { baseUrl: 'https://example.test', fetchImpl });
  assert.equal(asset.name, 'mskey_0.1.0_amd64.deb');
  files['mskey_0.1.0_amd64.deb'] = Buffer.from('tampered');
  await assert.rejects(fetchVerifiedAsset('linux-x64-deb', { baseUrl: 'https://example.test', fetchImpl }), /Checksum mismatch/);
});

test('reports streamed bytes without changing downloaded data', async () => {
  const updates = [];
  const fetchImpl = async () => new Response(new ReadableStream({
    start(controller) {
      controller.enqueue(new Uint8Array([1, 2]));
      controller.enqueue(new Uint8Array([3, 4, 5]));
      controller.close();
    },
  }), { headers: { 'content-length': '5' } });
  const data = await download('https://example.test/file', fetchImpl, (value) => updates.push(value));
  assert.deepEqual(data, Buffer.from([1, 2, 3, 4, 5]));
  assert.deepEqual(updates, [{ received: 0, total: 5 }, { received: 2, total: 5 }, { received: 5, total: 5 }]);
});

test('does not invent percentages for unknown or compressed lengths', async () => {
  for (const headers of [{}, { 'content-length': '10', 'content-encoding': 'gzip' }]) {
    const updates = [];
    await download('https://example.test/file', async () => new Response('data', { headers }), (value) => updates.push(value));
    assert.ok(updates.every((value) => value.total === null));
    assert.equal(updates.at(-1).received, 4);
  }
});

test('rejects interrupted response streams', async () => {
  await assert.rejects(download('https://example.test/file', async () => new Response(new ReadableStream({
    pull(controller) { controller.error(new Error('connection lost')); },
  }))), /connection lost/);
});
