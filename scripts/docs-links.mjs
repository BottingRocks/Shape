#!/usr/bin/env node
// Check relative links and heading anchors in tracked documentation.
//
// This intentionally uses only the Node.js standard library. It is a docs gate,
// not a general Markdown parser: fenced code blocks and inline code are skipped,
// while links whose label is code-formatted (for example [`path`](target.md))
// are still checked.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const EXTERNAL_SCHEME = /^[a-z][a-z0-9+.-]*:/i;

export function slugify(text) {
  return text
    .toLowerCase()
    .replace(/<[^>]*>/g, '')
    .replace(/[^\p{L}\p{N}\s_-]/gu, '')
    .trim()
    .replace(/\s/g, '-');
}

function stripInlineMarkdown(text) {
  return text
    .replace(/`([^`]*)`/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/[*~]/g, '');
}

function fenceOpening(line) {
  const match = line.match(/^ {0,3}(`{3,}|~{3,})/);
  if (!match) return null;
  return { char: match[1][0], length: match[1].length };
}

function fencedLines(lines) {
  const mask = new Array(lines.length).fill(false);
  let fence = null;
  for (let index = 0; index < lines.length; index += 1) {
    const opening = fenceOpening(lines[index]);
    if (fence) {
      mask[index] = true;
      if (
        opening &&
        opening.char === fence.char &&
        opening.length >= fence.length
      ) {
        fence = null;
      }
      continue;
    }
    if (opening) {
      fence = opening;
      mask[index] = true;
    }
  }
  return mask;
}

// Inline code may span lines only when a matching closing run exists later in
// the document. An unmatched run is literal text, so it must not hide links
// that follow it. Each returned range is a half-open [start, end) span.
function scanInlineCode(line, openRun, hasClosingRunLater) {
  const ranges = [];
  let index = 0;
  let open = openRun;

  if (open > 0) {
    const close = line.indexOf('`'.repeat(open));
    if (close === -1) {
      return { ranges: [[0, line.length]], open };
    }
    ranges.push([0, close + open]);
    index = close + open;
    open = 0;
  }

  while (index < line.length) {
    if (line[index] !== '`') {
      index += 1;
      continue;
    }
    let run = 1;
    while (line[index + run] === '`') run += 1;
    const close = line.indexOf('`'.repeat(run), index + run);
    if (close === -1) {
      if (open === 0 && !hasClosingRunLater(run)) {
        index += run;
        continue;
      }
      ranges.push([index, line.length]);
      open = run;
      break;
    }
    ranges.push([index, close + run]);
    index = close + run;
  }
  return { ranges, open };
}

function insideRanges(position, ranges) {
  return ranges.some(([start, end]) => position >= start && position < end);
}

// Return every inline Markdown link in a document, skipping fenced code and
// inline code spans. Labels may themselves contain inline code.
export function extractLinks(text) {
  const links = [];
  const lines = text.split(/\r?\n/);
  const fenced = fencedLines(lines);
  let fence = null;
  let inComment = false;
  let openInline = 0;

  for (let lineNumber = 1; lineNumber <= lines.length; lineNumber += 1) {
    const original = lines[lineNumber - 1];
    const opening = fenceOpening(original);
    if (fence) {
      if (
        opening &&
        opening.char === fence.char &&
        opening.length >= fence.length
      ) {
        fence = null;
      }
      continue;
    }
    if (opening) {
      fence = opening;
      continue;
    }

    let line = original;
    if (inComment) {
      const end = line.indexOf('-->');
      if (end === -1) continue;
      line = line.slice(end + 3);
      inComment = false;
    }
    const commentStart = line.indexOf('<!--');
    if (commentStart !== -1) {
      const commentEnd = line.indexOf('-->', commentStart + 4);
      if (commentEnd === -1) {
        inComment = true;
        line = line.slice(0, commentStart);
      } else {
        line = line.slice(0, commentStart) + line.slice(commentEnd + 3);
      }
    }

    const scanned = scanInlineCode(
      line,
      openInline,
      (run) => {
        const needle = '`'.repeat(run);
        for (let later = lineNumber; later < lines.length; later += 1) {
          if (!fenced[later] && lines[later].includes(needle)) return true;
        }
        return false;
      },
    );
    openInline = scanned.open;
    const codeRanges = scanned.ranges;
    const pattern =
      /!?\[([^\]]*)\]\(\s*<?([^)>\s]+)>?(?:\s+["'][^"']*["'])?\s*\)/g;
    let match;
    while ((match = pattern.exec(line)) !== null) {
      if (insideRanges(match.index, codeRanges)) continue;
      links.push({
        line: lineNumber,
        label: match[1],
        target: match[2],
      });
    }
  }
  return links;
}

// Collect GitHub-style heading anchors plus explicit HTML id/name anchors.
export function collectAnchors(text) {
  const anchors = new Set();
  const lines = text.split(/\r?\n/);
  let fence = null;

  for (const line of lines) {
    const opening = fenceOpening(line);
    if (fence) {
      if (
        opening &&
        opening.char === fence.char &&
        opening.length >= fence.length
      ) {
        fence = null;
      }
      continue;
    }
    if (opening) {
      fence = opening;
      continue;
    }

    for (const match of line.matchAll(/\sid\s*=\s*["']([^"']+)["']|\sname\s*=\s*["']([^"']+)["']/gi)) {
      anchors.add(match[1] ?? match[2]);
    }

    const heading = line.match(/^ {0,3}(#{1,6})\s+(.*?)\s*#*\s*$/);
    if (!heading) continue;
    const base = slugify(stripInlineMarkdown(heading[2]));
    if (!base) continue;
    let slug = base;
    let suffix = 1;
    while (anchors.has(slug)) {
      slug = `${base}-${suffix}`;
      suffix += 1;
    }
    anchors.add(slug);
  }
  return anchors;
}

function decodeTarget(target) {
  try {
    return decodeURIComponent(target);
  } catch {
    return target;
  }
}

function splitTarget(target) {
  const hash = target.indexOf('#');
  if (hash === -1) return { file: target, fragment: '' };
  return {
    file: target.slice(0, hash),
    fragment: target.slice(hash + 1),
  };
}

const anchorCache = new Map();

function anchorsFor(file) {
  if (!anchorCache.has(file)) {
    anchorCache.set(file, collectAnchors(fs.readFileSync(file, 'utf8')));
  }
  return anchorCache.get(file);
}

export function checkFile(root, relative) {
  const absolute = path.resolve(root, relative);
  const text = fs.readFileSync(absolute, 'utf8');
  const errors = [];

  for (const link of extractLinks(text)) {
    const target = decodeTarget(link.target);
    if (!target || target.startsWith('//') || EXTERNAL_SCHEME.test(target)) {
      continue;
    }

    const { file, fragment } = splitTarget(target);
    const targetFile = file
      ? path.resolve(path.dirname(absolute), file)
      : absolute;

    if (!fs.existsSync(targetFile)) {
      errors.push({
        file: relative,
        line: link.line,
        target: link.target,
        message: `missing link target: ${target}`,
      });
      continue;
    }

    if (!fragment || path.extname(targetFile).toLowerCase() !== '.md') {
      continue;
    }

    if (fs.statSync(targetFile).isDirectory()) continue;
    if (!anchorsFor(targetFile).has(fragment)) {
      errors.push({
        file: relative,
        line: link.line,
        target: link.target,
        message: `missing anchor "#${fragment}" in ${path.relative(root, targetFile)}`,
      });
    }
  }

  return errors;
}

export function defaultDocs(root) {
  const output = execFileSync(
    'git',
    ['-C', root, 'ls-files', '-z', '--', '*.md'],
    { encoding: 'buffer' },
  );
  return output
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .filter(
      (file) =>
        file === 'README.md' ||
        file.startsWith('docs/') ||
        file.startsWith('sdk/') ||
        file.startsWith('scripts/') ||
        file.startsWith('src/') && file.endsWith('/README.md') ||
        file.startsWith('cli/') && file.endsWith('/README.md') ||
        file.startsWith('tools/') ||
        (file.startsWith('tests/') && file.endsWith('/README.md')),
    )
    .sort();
}

export function runCheck(root = REPO_ROOT, files = null) {
  const targets = files && files.length > 0 ? files : defaultDocs(root);
  const errors = [];
  for (const file of targets) errors.push(...checkFile(root, file));
  return { targets, errors };
}

function main(argv) {
  let root = REPO_ROOT;
  const files = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--root') {
      root = path.resolve(argv[index + 1]);
      index += 1;
    } else if (argv[index] === '--help' || argv[index] === '-h') {
      process.stdout.write(
        'usage: node scripts/docs-links.mjs [--root DIR] [FILE ...]\n',
      );
      return 0;
    } else {
      files.push(argv[index]);
    }
  }

  const { targets, errors } = runCheck(root, files);
  for (const error of errors) {
    process.stderr.write(
      `${error.file}:${error.line}: ${error.message}\n`,
    );
  }
  process.stdout.write(
    `checked ${targets.length} documentation file${targets.length === 1 ? '' : 's'}: ` +
      `${errors.length} broken link${errors.length === 1 ? '' : 's'}\n`,
  );
  return errors.length === 0 ? 0 : 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
