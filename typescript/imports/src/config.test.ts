import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { expect, it } from 'vitest';

import { findConfig, parseConfig } from './config.ts';

const fixtures = fileURLToPath(
  new URL('../../../crates/stomme-cli/tests/fixtures/module-table', import.meta.url),
);

const packageVersion = (): string => {
  const manifest: unknown = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
  );

  if (typeof manifest !== 'object' || manifest === null || !('version' in manifest)) {
    throw new TypeError('package.json has no version');
  }

  const { version } = manifest;

  if (typeof version !== 'string') {
    throw new TypeError('package.json has no string version');
  }

  return version;
};

const validConfig = (extra = '') =>
  `schema = 1\nversion = "${packageVersion()}"\nlanguage = "typescript"\n${extra}`;

const refusal = (text: string): string => {
  const result = parseConfig(text, '/project');

  if (result.ok) {
    throw new Error('expected a refusal');
  }

  return result.reason;
};

const withTempFolder = (run: (folder: string) => void) => {
  const folder = mkdtempSync(join(tmpdir(), 'stomme-imports-'));

  try {
    run(folder);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
};

const caseFolders = (kind: 'accept' | 'reject') =>
  readdirSync(join(fixtures, kind), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);

it('accepts and refuses the shared stomme.toml fixtures like the binary', () => {
  for (const kind of ['accept', 'reject'] as const) {
    const cases = caseFolders(kind);

    expect(cases, `${kind} has cases`).not.toHaveLength(0);

    for (const name of cases) {
      const folder = join(fixtures, kind, name);
      const text = readFileSync(join(folder, 'stomme.toml'), 'utf8');
      const result = parseConfig(text, folder);
      const detail = result.ok ? 'accepted' : result.reason;

      expect(result.ok, `${kind}/${name}: ${detail}`).toBe(kind === 'accept');
    }
  }
});

it('refuses a rust project', () => {
  const config = validConfig().replace('language = "typescript"', 'language = "rust"');
  const reason = refusal(config);

  expect(reason).toContain('rust');
  expect(reason).toContain('typescript');
});

it('refuses a version other than the package version', () => {
  const config = validConfig().replace(`version = "${packageVersion()}"`, 'version = "9.9.9"');
  const reason = refusal(config);

  expect(reason).toContain('9.9.9');
  expect(reason).toContain(packageVersion());
});

it('refuses invalid TOML', () => {
  const reason = refusal('schema =');

  expect(reason).toContain('is not valid TOML');
  expect(reason).not.toMatch(/^is not valid TOML: (?:[A-Za-z]+)?Error:/);
});

it('refuses a missing schema', () => {
  const text = `version = "${packageVersion()}"\nlanguage = "typescript"`;

  expect(refusal(text)).toContain('is missing `schema`');
});

it('refuses a schema other than 1', () => {
  const config = validConfig().replace('schema = 1', 'schema = 2');

  expect(refusal(config)).toContain('schema = 2');
});

it('refuses a float schema', () => {
  const config = validConfig().replace('schema = 1', 'schema = 1.0');
  const reason = refusal(config);

  expect(reason).toContain('not an integer');
});

it('refuses an exponent schema', () => {
  const config = validConfig().replace('schema = 1', 'schema = 1e0');
  const reason = refusal(config);

  expect(reason).toContain('not an integer');
});

it('refuses a list schema without throwing', () => {
  const text = validConfig().replace('schema = 1', 'schema = [1]');

  expect(() => parseConfig(text, '/project')).not.toThrow();
  expect(refusal(text)).toContain('not an integer');
});

it('refuses a table schema without throwing', () => {
  const text = validConfig().replace('schema = 1', 'schema = { value = 1 }');

  expect(() => parseConfig(text, '/project')).not.toThrow();
  expect(refusal(text)).toContain('not an integer');
});

it('refuses a missing version', () => {
  const text = `schema = 1\nlanguage = "typescript"`;

  expect(refusal(text)).toContain('is missing `version`');
});

it('refuses a non-string version', () => {
  const text = `schema = 1\nversion = 1\nlanguage = "typescript"`;

  expect(refusal(text)).toContain('`version` that is not a string');
});

it('refuses a list version without throwing', () => {
  const text = `schema = 1\nversion = [1]\nlanguage = "typescript"`;

  expect(() => parseConfig(text, '/project')).not.toThrow();
  expect(refusal(text)).toContain('`version` that is not a string');
});

it('refuses a missing language', () => {
  const text = `schema = 1\nversion = "${packageVersion()}"`;

  expect(refusal(text)).toContain('is missing `language`');
});

it('refuses a non-string language', () => {
  const text = `schema = 1\nversion = "${packageVersion()}"\nlanguage = 1`;

  expect(refusal(text)).toContain('`language` that is not a string');
});

it('refuses a list language without throwing', () => {
  const text = `schema = 1\nversion = "${packageVersion()}"\nlanguage = [1]`;

  expect(() => parseConfig(text, '/project')).not.toThrow();
  expect(refusal(text)).toContain('`language` that is not a string');
});

it('refuses layers that are not a list of strings', () => {
  expect(refusal(validConfig('layers = [1]'))).toContain('`layers` that is not a list of strings');
});

it('refuses roles that are not a table of strings', () => {
  expect(refusal(validConfig('[roles]\ncore = 1'))).toContain(
    '`roles` that is not a table of strings',
  );
});

it('refuses modules that are not a table', () => {
  expect(refusal(validConfig('modules = "x"'))).toContain('`modules` that is not a table');
});

it('refuses a module row that is not a table', () => {
  expect(refusal(validConfig('[modules]\na = "x"'))).toContain('module `a` that is not a table');
});

it('refuses a module row with an unknown field', () => {
  const text = validConfig('[modules.a]\nentry = "src/a.ts"\nexports = []');

  expect(refusal(text)).toContain('exports');
});

it('refuses a missing entry', () => {
  expect(refusal(validConfig('[modules.a]\npublic = []'))).toContain('without a string `entry`');
});

it('refuses a non-string entry', () => {
  expect(refusal(validConfig('[modules.a]\nentry = 1'))).toContain('without a string `entry`');
});

it('refuses public files that are not a list of strings', () => {
  const text = validConfig('[modules.a]\nentry = "src/a.ts"\npublic = "src/a.ts"');

  expect(refusal(text)).toContain('`public` that is not a list of strings');
});

it('refuses imports that are not a list of strings', () => {
  const text = validConfig('[modules.a]\nentry = "src/a.ts"\nimports = "b"');

  expect(refusal(text)).toContain('`imports` that is not a list of strings');
});

it('refuses an absolute entry path', () => {
  expect(refusal(validConfig('[modules.a]\nentry = "/src/a.ts"'))).toContain('absolute');
});

it('refuses an absolute public path', () => {
  const text = validConfig('[modules.a]\nentry = "src/a.ts"\npublic = ["/src/a.ts"]');

  expect(refusal(text)).toContain('absolute');
});

it('refuses an entry path with a parent part', () => {
  expect(refusal(validConfig('[modules.a]\nentry = "../src/a.ts"'))).toContain('`..` part');
});

it('refuses a public path with a parent part', () => {
  const text = validConfig('[modules.a]\nentry = "src/a.ts"\npublic = ["src/../a.ts"]');

  expect(refusal(text)).toContain('`..` part');
});

it('refuses an import of a module with no row', () => {
  const text = validConfig('[modules.config]\nentry = "src/config.ts"\nimports = ["errors"]');
  const reason = refusal(text);

  expect(reason).toContain('`errors`');
  expect(reason).toContain('no `[modules.errors]` row');
});

it('ignores other top-level keys', () => {
  const result = parseConfig(
    validConfig('[exceptions]\nextra_docs = ["docs/extra.md"]'),
    '/project',
  );

  expect(result.ok).toBe(true);
});

it('reads rows with their entry, public files, and imports', () => {
  const text = validConfig(
    '[modules.errors]\nentry = "src/errors.ts"\n\n[modules.config]\nentry = "src/config/config.ts"\npublic = ["src/config/models.ts"]\nimports = ["errors"]',
  );

  const result = parseConfig(text, '/project');

  expect(result.ok).toBe(true);

  if (!result.ok) {
    return;
  }

  expect(result.config.root).toBe('/project');
  expect([...result.config.modules.keys()]).toEqual(['config', 'errors']);

  expect(result.config.modules.get('config')).toEqual({
    name: 'config',
    entry: 'src/config/config.ts',
    public: ['src/config/models.ts'],
    imports: ['errors'],
  });

  expect(result.config.modules.get('errors')).toEqual({
    name: 'errors',
    entry: 'src/errors.ts',
    public: [],
    imports: [],
  });
});

it('finds the nearest stomme.toml above a folder', () => {
  withTempFolder((folder) => {
    const nested = join(folder, 'a', 'b');

    mkdirSync(nested, { recursive: true });
    writeFileSync(join(folder, 'stomme.toml'), validConfig());

    const result = findConfig(nested);

    expect(result?.ok).toBe(true);

    if (result?.ok !== true) {
      return;
    }

    expect(result.config.root).toBe(folder);
  });
});

it('gives undefined without a stomme.toml', () => {
  withTempFolder((folder) => {
    expect(findConfig(folder)).toBeUndefined();
  });
});

it('refuses an unreadable file', () => {
  withTempFolder((folder) => {
    mkdirSync(join(folder, 'stomme.toml'));

    const result = findConfig(folder);

    expect(result?.ok).toBe(false);

    if (result?.ok !== false) {
      return;
    }

    expect(result.reason).toContain('could not read the file:');
    expect(result.reason).not.toMatch(/^could not read the file: (?:[A-Za-z]+)?Error:/);
  });
});

it('sees an edit after the file changes', () => {
  withTempFolder((folder) => {
    const path = join(folder, 'stomme.toml');

    writeFileSync(path, validConfig());
    expect(findConfig(folder)?.ok).toBe(true);

    const otherVersion = validConfig().replace(
      `version = "${packageVersion()}"`,
      'version = "9.9.9"',
    );

    writeFileSync(path, otherVersion);

    utimesSync(path, new Date(), new Date(Date.now() + 10_000));

    expect(findConfig(folder)?.ok).toBe(false);
  });
});
