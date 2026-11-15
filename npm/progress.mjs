export function createProgress(label, output = process.stderr, env = process.env) {
  const animate = Boolean(output.isTTY) && env.TERM !== 'dumb' && env.MSKEY_REDUCE_MOTION !== '1';
  const color = animate && env.NO_COLOR === undefined;
  const frames = ['◜', '◠', '◝', '◞', '◡', '◟'];
  let frame = 0;
  let state = { received: 0, total: null };
  let finished = false;
  const paint = () => {
    const { received, total } = state;
    const ratio = total ? Math.min(received / total, 1) : null;
    const size = `${(received / 1048576).toFixed(1)} MB`;
    const count = ratio === null ? 0 : Math.floor(ratio * 16);
    const meter = ratio === null ? size : `[${'━'.repeat(count)}${'─'.repeat(16 - count)}] ${Math.floor(ratio * 100)}% · ${size}`;
    const symbol = frames[frame++ % frames.length];
    const brand = color ? '\x1b[38;2;236;48;92mMS\x1b[38;2;0;163;237m Key\x1b[0m' : 'MS Key';
    const width = Math.max(1, (output.columns || 100) - 1);
    const text = `${symbol} MS Key · ${label} ${meter}`.slice(0, width);
    output.write(`\r\x1b[2K${color ? text.replace('MS Key', brand) : text}`);
  };
  if (animate) paint();
  else output.write(`MS Key · ${label}\n`);
  const timer = animate ? setInterval(paint, 90) : null;
  timer?.unref();
  return {
    update(value) { if (!finished) state = value; },
    finish(message) {
      if (finished) return;
      finished = true;
      if (timer) clearInterval(timer);
      if (animate) output.write('\r\x1b[2K');
      output.write(`MS Key · ${message}\n`);
    },
  };
}

export async function withProgress(label, work, output, env) {
  const progress = createProgress(label, output, env);
  try {
    const result = await work(progress.update);
    progress.finish('Download verified');
    return result;
  } catch (error) {
    progress.finish('Download failed');
    throw error;
  }
}
