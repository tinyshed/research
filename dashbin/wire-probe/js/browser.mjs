// the rounds in a browser tab: fetch every file, then run, then print into the page
import { bench, files, render, verify } from './run.mjs';

const out = document.getElementById('out');
try {
  const bytes = {};
  for (const name of files) bytes[name] = await (await fetch(`/out/${name}`)).arrayBuffer();
  const engine = `${navigator.userAgent}, crossOriginIsolated ${globalThis.crossOriginIsolated}`;
  const digests = verify(bytes);
  out.textContent = render(engine, digests, bench(bytes, () => performance.now()));
} catch (e) {
  out.textContent = `failed: ${e.stack ?? e}`;
}
out.dataset.done = '1';
