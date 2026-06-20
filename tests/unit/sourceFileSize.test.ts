import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

const SOURCE_ROOTS = ['src', 'src-tauri/src', 'tests'];
const SOURCE_EXTENSIONS = new Set(['.rs', '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs']);
const MAX_SOURCE_LINES = 800;

function extensionOf(path: string): string {
  const index = path.lastIndexOf('.');
  return index === -1 ? '' : path.slice(index);
}

function isExcluded(path: string): boolean {
  const normalized = path.split(sep).join('/');
  return /(^|\/)(generated|schema|templates?|fixtures?)(\/|$)/i.test(normalized);
}

function collectSourceFiles(root: string): string[] {
  const out: string[] = [];
  const visit = (path: string) => {
    if (isExcluded(relative(process.cwd(), path))) return;
    const stat = statSync(path);
    if (stat.isDirectory()) {
      for (const entry of readdirSync(path)) visit(join(path, entry));
      return;
    }
    if (SOURCE_EXTENSIONS.has(extensionOf(path))) out.push(path);
  };
  visit(join(process.cwd(), root));
  return out;
}

function lineCount(path: string): number {
  const contents = readFileSync(path, 'utf8');
  if (contents.length === 0) return 0;
  return contents.endsWith('\n') ? contents.split('\n').length - 1 : contents.split('\n').length;
}

describe('source file size', () => {
  it(`keeps source files at or below ${MAX_SOURCE_LINES} lines`, () => {
    const tooLarge = SOURCE_ROOTS.flatMap(collectSourceFiles)
      .map((path) => ({
        path: relative(process.cwd(), path),
        lines: lineCount(path),
      }))
      .filter((entry) => entry.lines > MAX_SOURCE_LINES)
      .sort((a, b) => b.lines - a.lines);

    expect(tooLarge).toEqual([]);
  });
});
