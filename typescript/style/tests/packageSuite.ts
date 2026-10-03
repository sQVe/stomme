import { mkdir, mkdtemp, readFile, realpath, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';

import { afterAll, beforeAll, describe, inject, it } from 'vitest';

import type { Consumer, Runtime } from './consumer.ts';
import { installConsumer, vitePlusSeenByPackage, writeFiles } from './consumer.ts';

type CaseFolder = (
  files: Record<string, string>,
  onTestFinished: (cleanup: () => Promise<void>) => void,
  base?: string,
) => Promise<{ folder: string; directory: string }>;

export interface PackageContext {
  readonly consumer: Consumer;
  caseFolder: CaseFolder;
}

export type PackageTests = (context: PackageContext) => void;

const lines = (...text: string[]) => `${text.join('\n')}\n`;

const linesWith = (output: string, text: string) =>
  output.split('\n').filter((line) => line.includes(text));

// On GitHub Actions the linter also prints a run summary when it finds nothing.
const summaryPattern = /^(?:Found \d+ warnings? and \d+ errors?\.|Finished in .*)$/;

const diagnosticOutput = (output: string) =>
  output
    .split('\n')
    .filter((line) => line.trim() !== '' && !summaryPattern.test(line))
    .join('\n');

// Each test spawns up to three commands, and a fix runs lint and the formatter.
const spawnedTestTimeout = 120_000;

// These tests load the package's config, plugin, and presets, so they run under every runtime.
export const loadingTests: PackageTests = (context) => {
  it('runs the consumer copy of Vite Plus', async ({ expect }) => {
    const consumerVitePlus = await realpath(
      join(context.consumer.directory, 'node_modules/vite-plus'),
    );

    expect(await vitePlusSeenByPackage(context.consumer.directory)).toBe(consumerVitePlus);
  });

  it('enforces house rules only through the seam command', async ({ expect, onTestFinished }) => {
    const { folder } = await context.caseFolder(
      {
        'style.ts': lines(
          '// See ADR 0012.',
          'export const MAX_RETRIES = 3;',
          'export interface requestOptions { request_id: string }',
          'export const cb = (): number => 1;',
          'export const caller = (): number => helper();',
          'const helper = (): number => 1;',
          'export const normalize = (name: string): string => {',
          '  const trimmed = name.trim(); // Keep with the declaration.',
          '  return trimmed;',
          '};',
          'export const width = 80, height = 24;',
          'export const read = (value: string): string => {',
          '  let result: string;',
          '  if ((result = value)) { return result; }',
          "  return '';",
          '};',
          '// eslint-disable-next-line no-console',
          "console.info('started');",
          'export type Payload = unknown;',
          "export const label = 'a' as string;",
        ),
        'fake.test.ts': lines("export const fakeLabel = 'a' as string;"),
        'view.tsx': lines(
          'export const MAIN_VIEW = (): unknown => <box />;',
          'export const NotView = (): number => 1;',
        ),
      },
      onTestFinished,
    );

    const ordinary = await context.consumer.ordinaryLint([folder]);
    const style = await context.consumer.seam([folder]);

    expect(ordinary.error).toBeUndefined();
    expect(diagnosticOutput(ordinary.stdout)).toBe('');
    expect(ordinary.status).toBe(0);
    expect(style.error).toBeUndefined();
    expect(style.status).toBe(1);

    for (const rule of [
      'seam(naming-convention)',
      'seam(no-abbreviations)',
      'seam(helper-before-use)',
      'seam(no-reference-comments)',
      'seam(require-disable-reason)',
      'seam(no-unknown-type-aliases)',
      'seam(require-safety-comment-for-type-assertion)',
      'padding-line-between-statements',
      'one-var',
      'no-cond-assign',
    ]) {
      expect(ordinary.stdout).not.toContain(rule);
      expect(style.stdout).toContain(rule);
    }

    expect(
      linesWith(style.stdout, 'view.tsx:')
        .filter((line) => line.includes('seam(naming-convention)'))
        .map((line) => /"(\w+)"/.exec(line)?.[1]),
    ).toEqual(expect.arrayContaining(['MAIN_VIEW', 'NotView']));

    expect(linesWith(style.stdout, 'fake.test.ts:')).toEqual([]);
  });

  it('runs a local plugin of the consumer beside the house rules', async ({
    expect,
    onTestFinished,
  }) => {
    const project = join(context.consumer.directory, 'local');

    await writeFiles(project, {
      'package.json': JSON.stringify({ name: 'local', private: true, type: 'module' }),
      'tsconfig.json': JSON.stringify({ extends: '../tsconfig.json' }),
      'vite.config.ts': lines(
        "import { lint } from '@sqve/seam';",
        "import { defineConfig } from 'vite-plus';",
        '',
        'export default defineConfig({',
        '  lint: {',
        '    extends: [lint],',
        "    jsPlugins: ['./localPlugin.js'],",
        "    rules: { 'local/no-forbidden-name': 'error' },",
        '  },',
        '});',
      ),
      'localPlugin.js': lines(
        'export default {',
        "  meta: { name: 'local' },",
        '  rules: {',
        "    'no-forbidden-name': {",
        "      meta: { type: 'problem', schema: [], messages: { forbidden: 'Rename this value.' } },",
        '      create(context) {',
        '        return {',
        '          Identifier(node) {',
        "            if (node.name === 'forbidden') {",
        "              context.report({ node, messageId: 'forbidden' });",
        '            }',
        '          },',
        '        };',
        '      },',
        '    },',
        '  },',
        '};',
      ),
      'cases/names.ts': lines('export const forbidden = 1;', 'export const MAX_RETRIES = 1;'),
    });

    onTestFinished(() => rm(project, { recursive: true, force: true }));

    const ordinary = await context.consumer.ordinaryLint(['cases'], project);
    const style = await context.consumer.seam(['cases'], project);

    expect(ordinary.error).toBeUndefined();
    expect(ordinary.status).toBe(1);
    expect(ordinary.stdout).toContain('local(no-forbidden-name)');
    expect(ordinary.stdout).not.toContain('seam(');
    expect(style.error).toBeUndefined();
    expect(style.status).toBe(1);
    expect(style.stdout).toContain('local(no-forbidden-name)');
    expect(style.stdout).toContain('seam(naming-convention)');
  });

  it('loads the React and Vitest presets through extends', async ({ expect, onTestFinished }) => {
    const files = {
      'hooks.tsx': lines(
        "import { useState } from 'react';",
        '',
        'export const Counter = (shown: boolean): unknown => {',
        '  if (shown) {',
        '    useState(0);',
        '  }',
        '',
        '  return null;',
        '};',
      ),
      'focused.test.ts': lines(
        "import { expect, it } from 'vitest';",
        '',
        "it.only('runs', () => {",
        '  expect(1).toBe(1);',
        '});',
      ),
      'mocked.test.ts': lines("import { vi } from 'vitest';", '', "vi.mock('./hooks.tsx');"),
    };

    const { folder } = await context.caseFolder(files, onTestFinished, 'presets');
    const presets = join(context.consumer.directory, 'presets');
    const withPresets = await context.consumer.seam([folder], presets);
    const base = await context.caseFolder(files, onTestFinished);
    const withoutPresets = await context.consumer.seam([base.folder]);

    expect(withPresets.stdout).toContain('(rules-of-hooks)');
    expect(withPresets.stdout).toContain('(no-focused-tests)');
    expect(withPresets.stdout).toContain('(prefer-import-in-mock)');
    expect(withoutPresets.stdout).not.toContain('rules-of-hooks');
    expect(withoutPresets.stdout).not.toContain('no-focused-tests');
    expect(withoutPresets.stdout).not.toContain('prefer-import-in-mock');
  });
};

// These tests check lint and fix results, which do not depend on the runtime that loads the package.
export const behaviorTests: PackageTests = (context) => {
  it('rejects lint warnings', async ({ expect, onTestFinished }) => {
    const { folder } = await context.caseFolder(
      { 'warning.ts': "console.log('warning fixture');\n" },
      onTestFinished,
    );

    const result = await context.consumer.seam([folder]);

    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(result.stdout).toContain('eslint(no-console)');
  });

  it('lints only the paths it is given', async ({ expect, onTestFinished }) => {
    const { folder } = await context.caseFolder(
      {
        'clean.ts': 'export const retries = 3;\n',
        'dirty.ts': 'export const MAX_RETRIES = 3;\n',
      },
      onTestFinished,
    );

    const clean = await context.consumer.seam([`${folder}/clean.ts`]);
    const whole = await context.consumer.seam([folder]);

    expect(clean.status).toBe(0);
    expect(whole.status).toBe(1);

    expect(linesWith(whole.stdout, 'naming-convention')).toEqual([
      expect.stringContaining('dirty.ts:'),
    ]);
  });

  it('fixes house spacing, repeats the fix unchanged, and formats after a failed fix', async ({
    expect,
    onTestFinished,
  }) => {
    const { directory, folder } = await context.caseFolder(
      {
        'spacing.ts': lines(
          'export const normalize = (name: string): string => {',
          '  const trimmed = name.trim(); // Keep with the declaration.',
          '  // Keep with the guard.',
          '  if (!trimmed) {',
          "    return 'unknown';",
          '  }',
          '  return trimmed;',
          '};',
          'export const width = 80, height = 24;',
        ),
        'manual.ts':
          'export const MAX_RETRIES=3;\nexport const caller=():number=>helper();\nconst helper=():number=>1;\n',
      },
      onTestFinished,
    );

    const spacing = join(directory, 'spacing.ts');
    const fix = await context.consumer.seam(['--fix', `${folder}/spacing.ts`]);

    expect(fix.error).toBeUndefined();
    expect(fix.stdout + fix.stderr).not.toMatch(/\berror\b/);
    expect(fix.status).toBe(0);

    expect(await readFile(spacing, 'utf8')).toBe(
      lines(
        'export const normalize = (name: string): string => {',
        '  const trimmed = name.trim(); // Keep with the declaration.',
        '',
        '  // Keep with the guard.',
        '  if (!trimmed) {',
        "    return 'unknown';",
        '  }',
        '',
        '  return trimmed;',
        '};',
        '',
        'export const width = 80;',
        'export const height = 24;',
      ),
    );

    const fixed = await readFile(spacing, 'utf8');
    const repeated = await context.consumer.seam(['--fix', `${folder}/spacing.ts`]);

    expect(repeated.status).toBe(0);
    expect(await readFile(spacing, 'utf8')).toBe(fixed);

    const manual = await context.consumer.seam(['--fix', `${folder}/manual.ts`]);

    expect(manual.error).toBeUndefined();
    expect(manual.status).toBe(1);

    expect(await readFile(join(directory, 'manual.ts'), 'utf8')).toBe(
      lines(
        'export const MAX_RETRIES = 3;',
        'export const caller = (): number => helper();',
        'const helper = (): number => 1;',
      ),
    );
  });

  it('pads loop exits and multiline statements in a form the formatter keeps', async ({
    expect,
    onTestFinished,
  }) => {
    const { directory, folder } = await context.caseFolder(
      {
        'multiline.ts': lines(
          'export const limit = 9;',
          'export const options = {',
          '  limit,',
          '};',
          '',
          'export const collect = (values: number[]): number[] => {',
          '  const collected: number[] = [];',
          '',
          '  for (const value of values) {',
          '    if (value < 0) {',
          '      collected.push(0);',
          '      continue;',
          '    }',
          '',
          '    if (value > limit) {',
          '      collected.push(limit);',
          '      break;',
          '    }',
          '',
          '    collected.push(value);',
          '  }',
          '',
          '  collected.sort((left, right) => left - right);',
          '  Object.assign(collected, {',
          '    total: collected.length,',
          '  });',
          '  collected.reverse();',
          '',
          '  return collected;',
          '};',
        ),
      },
      onTestFinished,
    );

    const check = await context.consumer.seam([folder]);

    expect(check.status).toBe(1);
    expect(linesWith(check.stdout, 'padding-line-between-statements')).toHaveLength(5);

    const fix = await context.consumer.seam(['--fix', folder]);

    expect(fix.status).toBe(0);

    expect(await readFile(join(directory, 'multiline.ts'), 'utf8')).toBe(
      lines(
        'export const limit = 9;',
        '',
        'export const options = {',
        '  limit,',
        '};',
        '',
        'export const collect = (values: number[]): number[] => {',
        '  const collected: number[] = [];',
        '',
        '  for (const value of values) {',
        '    if (value < 0) {',
        '      collected.push(0);',
        '',
        '      continue;',
        '    }',
        '',
        '    if (value > limit) {',
        '      collected.push(limit);',
        '',
        '      break;',
        '    }',
        '',
        '    collected.push(value);',
        '  }',
        '',
        '  collected.sort((left, right) => left - right);',
        '',
        '  Object.assign(collected, {',
        '    total: collected.length,',
        '  });',
        '',
        '  collected.reverse();',
        '',
        '  return collected;',
        '};',
      ),
    );

    expect((await context.consumer.seam([folder])).status).toBe(0);
  });

  it('passes a long function, a long file, and five parameters', async ({
    expect,
    onTestFinished,
  }) => {
    const { folder } = await context.caseFolder(
      {
        'large.ts': lines(
          'export const sum = (first: number, second: number, third: number, fourth: number, fifth: number): number => first + second + third + fourth + fifth;',
          '',
          'export const longFunction = (values: number[]): void => {',
          ...Array.from({ length: 61 }, () => '  values.push(values.length);'),
          '};',
          '',
          ...Array.from({ length: 501 }, (_, index) => `export const value${index} = ${index};`),
        ),
      },
      onTestFinished,
    );

    const result = await context.consumer.seam([folder]);

    expect(diagnosticOutput(result.stdout)).toBe('');
    expect(result.status).toBe(0);
  });

  it('type-checks consumer code against the declarations without skipping libraries', async ({
    expect,
    onTestFinished,
  }) => {
    const compilerOptions = {
      strict: true,
      module: 'NodeNext',
      moduleResolution: 'NodeNext',
      target: 'ES2024',
      lib: ['ES2024'],
      noEmit: true,
      skipLibCheck: false,
      types: [],
    };

    const { folder } = await context.caseFolder(
      {
        'usage.ts': lines(
          "import { format, lint, react, vitest } from '@sqve/seam';",
          "import plugin from '@sqve/seam/plugin';",
          "import type { OxlintConfig } from 'vite-plus/lint';",
          '',
          'export const configs: OxlintConfig[] = [lint, react, vitest];',
          'export const printWidth: number | undefined = format.printWidth;',
          'export const ruleNames: string[] = Object.keys(plugin.rules);',
        ),
        'misuse.ts': lines(
          "import { lint } from '@sqve/seam';",
          '',
          'export const wrong: number = lint;',
        ),
        'tsconfig.usage.json': JSON.stringify({ compilerOptions, files: ['usage.ts'] }),
        'tsconfig.misuse.json': JSON.stringify({ compilerOptions, files: ['misuse.ts'] }),
      },
      onTestFinished,
    );

    const usage = await context.consumer.typecheck(join(folder, 'tsconfig.usage.json'));
    const misuse = await context.consumer.typecheck(join(folder, 'tsconfig.misuse.json'));

    expect(usage.error).toBeUndefined();
    expect(usage.stdout).toBe('');
    expect(usage.status).toBe(0);
    // A failing misuse proves the declarations resolved to real types, not `any`.
    expect(misuse.status).not.toBe(0);
    expect(misuse.stdout).toContain("Type 'OxlintConfig' is not assignable to type 'number'");
  });

  it('reports each always-on rule added by this package', async ({ expect, onTestFinished }) => {
    const fixtures: Record<string, [rule: string, source: string]> = {
      'arrayCallbackReturn.ts': [
        'eslint(array-callback-return)',
        "export const lengths = ['a'].map((value) => { if (value) { return value.length; } });",
      ],
      'uselessAssignment.ts': [
        'eslint(no-useless-assignment)',
        'export const pick = (): number => { let value = 1; value = 2; return value; };',
      ],
      'var.ts': ['eslint(no-var)', 'export var count = 1;'],
      'returnAssign.ts': [
        'eslint(no-return-assign)',
        'let total = 0;\nexport const add = (value: number): number => { return total += value; };',
      ],
      'multiAssign.ts': [
        'eslint(no-multi-assign)',
        'let first = 0;\nlet second = 0;\nfirst = second = 1;\nexport { first, second };',
      ],
      'sequences.ts': [
        'eslint(no-sequences)',
        'let first = 0;\nlet second = 0;\nfirst++, second++;\nexport { first, second };',
      ],
      'loopFunction.ts': [
        'eslint(no-loop-func)',
        'export const callbacks: (() => number)[] = [];\nlet index = 0;\nwhile (index < callbacks.length) { callbacks.push(() => index); index++; }',
      ],
      'executorReturn.ts': [
        'eslint(no-promise-executor-return)',
        'export const wait = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));',
      ],
      'fallthrough.ts': [
        'eslint(no-fallthrough)',
        "export const label = (value: number): string => { let text = ''; switch (value) { case 0: text = 'zero'; case 1: text = 'one'; break; default: text = 'other'; } return text; };",
      ],
      'unboundMethod.ts': [
        'typescript(unbound-method)',
        'class Counter { count = 0; increment(): void { this.count++; } }\nexport const increment = new Counter().increment;',
      ],
      'enumComparison.ts': [
        'typescript(no-unsafe-enum-comparison)',
        "enum Color { Red = 'red' }\nexport const isRed = (value: string): boolean => value === Color.Red;",
      ],
      'alwaysReturn.ts': [
        'promise(always-return)',
        'export const loaded = Promise.resolve(1).then((value) => { console.info(value); }).then(() => 1);',
      ],
      'catchOrReturn.ts': [
        'promise(catch-or-return)',
        'Promise.resolve(1).then((value) => value);',
      ],
      'returnWrap.ts': [
        'promise(no-return-wrap)',
        'export const wrapped = Promise.resolve(1).then((value) => Promise.resolve(value));',
      ],
      'unusedDirective.ts': [
        'Unused eslint-disable directive',
        '// eslint-disable-next-line no-console -- Prints nothing.\nexport const quiet = 1;',
      ],
      'magicNumber.ts': [
        'eslint(no-magic-numbers)',
        'export const delay = (seconds: number): number => seconds * 1000;',
      ],
      'boundaryTypes.ts': [
        'typescript(explicit-module-boundary-types)',
        'export const twice = (value: number) => value + value;',
      ],
      'voidPromise.ts': [
        'typescript(no-floating-promises)',
        "export const start = (): void => { void Promise.reject(new Error('stopped')); };",
      ],
      'methodSignature.ts': [
        'typescript(method-signature-style)',
        'export interface Store { read(): string }',
      ],
      'extraneousClass.ts': [
        'typescript(no-extraneous-class)',
        'export class Paths { static readonly root = "/"; }',
      ],
      'lengthCheck.ts': [
        'unicorn(explicit-length-check)',
        'export const isEmpty = (items: string[]): boolean => !items.length;',
      ],
    };

    const files = Object.fromEntries(
      Object.entries(fixtures).map(([name, [, source]]) => [name, `${source}\n`]),
    );

    const { folder } = await context.caseFolder(files, onTestFinished);
    const result = await context.consumer.seam([folder]);

    expect(result.status).toBe(1);

    for (const [name, [rule]] of Object.entries(fixtures)) {
      const diagnostics = linesWith(result.stdout, `${name}:`);

      expect(diagnostics, name).toEqual(expect.arrayContaining([expect.stringContaining(rule)]));
    }
  });
};

// Each runtime's test file installs its own consumer, and its tests run at the same time.
export const describePackage = (runtime: Runtime, groups: PackageTests[]): void => {
  describe.concurrent(
    `the built package under ${runtime.name}`,
    () => {
      let consumerDirectory = '';
      let consumer: Consumer;

      // Each test writes its fixtures into its own folder inside the consumer project.
      const caseFolder: CaseFolder = async (files, onTestFinished, base = '') => {
        const project = join(consumer.directory, base);
        const cases = join(project, 'cases');

        await mkdir(cases, { recursive: true });

        const directory = await mkdtemp(join(cases, 'case-'));
        const folder = relative(project, directory);

        await writeFiles(directory, files);
        onTestFinished(() => rm(directory, { recursive: true, force: true }));

        return { folder, directory };
      };

      const context: PackageContext = {
        get consumer() {
          return consumer;
        },
        caseFolder,
      };

      beforeAll(async () => {
        consumerDirectory = await mkdtemp(join(tmpdir(), 'seam-consumer-'));
        consumer = await installConsumer(runtime, inject('tarball'), consumerDirectory);
      }, 300_000);

      afterAll(async () => {
        await rm(consumerDirectory, { recursive: true, force: true });
      });

      for (const group of groups) {
        group(context);
      }
    },
    spawnedTestTimeout,
  );
};
