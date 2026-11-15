import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fetchVerifiedAsset, parseChecksums, pickAsset, sha256, targetKey } from './release.mjs';

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
