// small.mjs in a browser tab
import { formats } from './run.mjs';
import { datasets, runSmall, sizes } from './small-run.mjs';

const out = document.getElementById('out');
try {
  const bytes = {};
  for (const n of sizes) {
    for (const d of datasets) {
      for (const { label } of formats) bytes[`${n}/${d}.${label}`] = await (await fetch(`/out-${n}/${d}.${label}`)).arrayBuffer();
    }
  }
  out.textContent = `${navigator.userAgent}\n${runSmall(bytes, () => performance.now())}`;
} catch (e) {
  out.textContent = `failed: ${e.stack ?? e}`;
}
out.dataset.done = '1';
