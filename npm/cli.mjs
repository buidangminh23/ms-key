#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DEFAULT_BASE_URL, REPO, download, fetchVerifiedAsset, parseChecksums, sha256, targetKey } from './release.mjs';
import { withProgress } from './progress.mjs';

const HELP = `MS Key installer

Usage:
  npx @minhspark/ms-key            Download and install the latest MS Key for this computer
  npx @minhspark/ms-key --download <folder>
                                   Only download and verify the installer into <folder>
  npx @minhspark/ms-key --version  Print this package version

Releases: https://github.com/${REPO}/releases
`;

function packageVersion() {
  const pkgPath = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'package.json');
  return JSON.parse(readFileSync(pkgPath, 'utf8')).version;
}

function hasCommand(command) {
  return spawnSync('sh', ['-c', `command -v ${command}`], { stdio: 'ignore' }).status === 0;
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with ${result.status}`);
}

async function installMac(baseUrl) {
  const script = await withProgress('Preparing macOS installer', async (onProgress) => {
    const sums = parseChecksums((await download(`${baseUrl}/SHA256SUMS`)).toString('utf8'));
    const data = await download(`${baseUrl}/install-macos.sh`, undefined, onProgress);
    if (sha256(data) !== sums.get('install-macos.sh')) throw new Error('Checksum mismatch for install-macos.sh');
    return data;
  });
  const dir = mkdtempSync(path.join(tmpdir(), 'mskey-'));
  try {
    const file = path.join(dir, 'install-macos.sh');
    writeFileSync(file, script, { mode: 0o755 });
    const result = spawnSync('bash', [file], { stdio: 'inherit', env: { ...process.env, MSKEY_BASE_URL: baseUrl } });
    if (result.status !== 0) throw new Error('install-macos.sh failed');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function installOther(key, baseUrl) {
  const asset = await withProgress('Downloading installer', (onProgress) => fetchVerifiedAsset(key, { baseUrl, onProgress }));
  const dir = mkdtempSync(path.join(tmpdir(), 'mskey-'));
  const file = path.join(dir, asset.name);
  try {
    writeFileSync(file, asset.data, { mode: 0o755 });
    console.log(`Verified ${asset.name} (SHA-256 ${asset.sha256.slice(0, 12)}...)`);
    if (key === 'win32-x64') {
      run(file, []);
    } else if (key === 'linux-x64-deb') {
      run('sudo', ['apt-get', 'install', '-y', file]);
      console.log("Run 'ibus restart', then add 'MS Key (Tiếng Việt)' in Settings > Keyboard > Input Sources.");
    } else {
      run('tar', ['-xzf', file, '-C', dir]);
      const folder = asset.name.replace(/\.tar\.gz$/, '');
      run('bash', [path.join(dir, folder, 'install.sh')]);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function downloadOnly(key, folder, baseUrl) {
  const asset = await withProgress('Downloading installer', (onProgress) => fetchVerifiedAsset(key, { baseUrl, onProgress }));
  mkdirSync(folder, { recursive: true });
  const file = path.resolve(folder, asset.name);
  writeFileSync(file, asset.data);
  console.log(`Downloaded and verified ${file}`);
}

async function main(argv) {
  if (argv.includes('-h') || argv.includes('--help')) return void process.stdout.write(HELP);
  if (argv.includes('--version')) return void console.log(packageVersion());
  const baseUrl = process.env.MSKEY_BASE_URL || DEFAULT_BASE_URL;
  const isLinux = process.platform === 'linux';
  const key = targetKey(process.platform, process.arch, isLinux && hasCommand('dpkg') && hasCommand('apt-get'));
  if (!key) throw new Error(`MS Key has no build for ${process.platform}/${process.arch}`);
  const index = argv.indexOf('--download');
  if (index !== -1) {
    const folder = argv[index + 1];
    if (!folder) throw new Error('--download needs a folder');
    return downloadOnly(key, folder, baseUrl);
  }
  if (key.startsWith('darwin')) return installMac(baseUrl);
  return installOther(key, baseUrl);
}

main(process.argv.slice(2)).catch((error) => {
  console.error(`MS Key: ${error.message}`);
  process.exit(1);
});
