import { createHash } from 'node:crypto';

export const REPO = 'buidangminh23/ms-key';
export const DEFAULT_BASE_URL = `https://github.com/${REPO}/releases/latest/download`;

const ASSET_PATTERNS = {
  'win32-x64': /^MSKey-Setup-[\w.+-]+-windows-x64\.exe$/,
  'darwin-arm64': /^MSKey-[\w.+-]+-macos-universal\.zip$/,
  'darwin-x64': /^MSKey-[\w.+-]+-macos-universal\.zip$/,
  'linux-x64-deb': /^mskey_[\w.+~-]+_amd64\.deb$/,
  'linux-x64-tar': /^MSKey-[\w.+-]+-linux-amd64\.tar\.gz$/,
};

export function parseChecksums(text) {
  const entries = new Map();
  for (const rawLine of text.replace(/\r/g, '').split('\n')) {
    const line = rawLine.trim();
    if (!line) continue;
    const match = /^([0-9a-f]{64})\s+\*?(\S+)$/i.exec(line);
    if (!match) throw new Error(`Malformed SHA256SUMS line: ${line}`);
    entries.set(match[2], match[1].toLowerCase());
  }
  return entries;
}

export function targetKey(platform, arch, hasDpkg) {
  if (platform === 'win32' && arch === 'x64') return 'win32-x64';
  if (platform === 'darwin' && (arch === 'arm64' || arch === 'x64')) return `darwin-${arch}`;
  if (platform === 'linux' && arch === 'x64') return hasDpkg ? 'linux-x64-deb' : 'linux-x64-tar';
  return null;
}

export function pickAsset(checksums, key) {
  const pattern = ASSET_PATTERNS[key];
  if (!pattern) throw new Error(`No MS Key build for ${key}`);
  const names = [...checksums.keys()].filter((name) => pattern.test(name));
  if (names.length !== 1) throw new Error(`Expected one ${key} asset in SHA256SUMS, found ${names.length}`);
  return { name: names[0], sha256: checksums.get(names[0]) };
}

export function sha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

export async function download(url, fetchImpl = globalThis.fetch) {
  const response = await fetchImpl(url, { redirect: 'follow', signal: AbortSignal.timeout(120_000) });
  if (!response.ok) throw new Error(`Download failed (${response.status}) for ${url}`);
  return Buffer.from(await response.arrayBuffer());
}

export async function fetchVerifiedAsset(key, { baseUrl = DEFAULT_BASE_URL, fetchImpl } = {}) {
  const sums = parseChecksums((await download(`${baseUrl}/SHA256SUMS`, fetchImpl)).toString('utf8'));
  const asset = pickAsset(sums, key);
  const data = await download(`${baseUrl}/${asset.name}`, fetchImpl);
  const actual = sha256(data);
  if (actual !== asset.sha256) throw new Error(`Checksum mismatch for ${asset.name}`);
  return { ...asset, data };
}
