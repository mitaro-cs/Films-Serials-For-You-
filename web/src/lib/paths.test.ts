import { readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

// На Mac и Windows регистр в именах файлов не различается: `./ask.svelte` там находит
// `Ask.svelte`, а не `ask.svelte.ts` (так сломалась сборка 0.1.0). В Linux CI этого не видно.
function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

describe('имена файлов', () => {
  it('не совпадают без учёта регистра, и модуль `x.svelte.ts` не спорит с компонентом `X.svelte`', () => {
    const root = join(__dirname, '..');
    const seen = new Map<string, string>();
    const clashes: string[] = [];
    for (const f of walk(root)) {
      const keys = [f.toLowerCase()];
      if (f.endsWith('.svelte.ts')) keys.push(f.slice(0, -3).toLowerCase());
      for (const k of keys) {
        const prev = seen.get(k);
        if (prev && prev !== f) clashes.push(`${relative(root, prev)} ↔ ${relative(root, f)}`);
        seen.set(k, f);
      }
    }
    expect(clashes).toEqual([]);
  });
});
