// Compile every TypeScript recipe and syntax-check every JavaScript recipe.
import { readFileSync, mkdtempSync, mkdirSync, writeFileSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = fileURLToPath(new URL('..', import.meta.url));
const scratch = mkdtempSync(join(tmpdir(), 'shape-node-docs-'));
try {
  mkdirSync(join(scratch, 'node_modules', '@types'), { recursive: true });
  symlinkSync(root, join(scratch, 'node_modules', 'shape'), 'dir');
  symlinkSync(resolve(root, 'node_modules/@types/node'), join(scratch, 'node_modules/@types/node'), 'dir');
  const types = [];
  let javascript = 0;
  for (const filename of ['README.md', 'TYPESCRIPT.md']) {
    const markdown = readFileSync(resolve(root, filename), 'utf8');
    let number = 0;
    for (const match of markdown.matchAll(/```(js|ts)\n([\s\S]*?)```/g)) {
      const language = match[1];
      const recipe = join(scratch, `${filename}-${++number}.${language === 'ts' ? 'mts' : 'mjs'}`);
      writeFileSync(recipe, match[2]);
      if (language === 'ts') types.push(recipe);
      else { execFileSync(process.execPath, ['--check', recipe], { stdio: 'pipe' }); javascript++; }
    }
  }
  execFileSync(process.execPath, [resolve(root, 'node_modules/typescript/bin/tsc'),
    '--noEmit', '--strict', '--noUncheckedIndexedAccess', '--target', 'ES2022',
    '--module', 'NodeNext', '--moduleResolution', 'NodeNext', ...types], { stdio: 'inherit' });
  console.log(`Checked ${javascript} JavaScript recipes and ${types.length} strict typed ESM recipes`);
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
