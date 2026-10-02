import { mkdtemp, readFile, realpath, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import type { Consumer } from './consumer.ts';
import {
  installConsumer,
  packPackage,
  runtimes,
  vitePlusSeenByPackage,
  writeFiles,
} from './consumer.ts';

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

let temporaryRoot = '';
let tarball = '';

beforeAll(async () => {
  temporaryRoot = await mkdtemp(join(tmpdir(), 'seam-package-'));
  tarball = await packPackage(join(temporaryRoot, 'pack'));
}, 300_000);

afterAll(async () => {
  await rm(temporaryRoot, { recursive: true, force: true });
});

describe.each(runtimes)(
  'the built package under $name',
  (runtime) => {
    let consumer: Consumer;
    let caseNumber = 0;

    // Each test writes its fixtures into its own folder inside the consumer project.
    const caseFolder = async (
      files: Record<string, string>,
      onTestFinished: (cleanup: () => Promise<void>) => void,
      base = '',
    ) => {
      caseNumber += 1;

      const folder = join('cases', `case${caseNumber}`);
      const directory = join(consumer.directory, base, folder);

      await writeFiles(directory, files);
      onTestFinished(() => rm(directory, { recursive: true, force: true }));

      return { folder, directory };
    };

    beforeAll(async () => {
      const directory = join(temporaryRoot, runtime.name.replaceAll(' ', '-'));

      consumer = await installConsumer(runtime, tarball, directory);
    }, 300_000);

    it('runs the consumer copy of Vite Plus', async () => {
      const consumerVitePlus = await realpath(join(consumer.directory, 'node_modules/vite-plus'));

      expect(await vitePlusSeenByPackage(consumer.directory)).toBe(consumerVitePlus);
    });

    it('rejects lint warnings', async ({ onTestFinished }) => {
      const { folder } = await caseFolder(
        { 'warning.ts': "console.log('warning fixture');\n" },
        onTestFinished,
      );

      const result = consumer.seam([folder]);

      expect(result.error).toBeUndefined();
      expect(result.status).toBe(1);
      expect(result.stdout).toContain('eslint(no-console)');
    });

    it('enforces house rules only through the seam command', async ({ onTestFinished }) => {
      const { folder } = await caseFolder(
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

      const ordinary = consumer.ordinaryLint([folder]);
      const style = consumer.seam([folder]);

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

    it('lints only the paths it is given', async ({ onTestFinished }) => {
      const { folder } = await caseFolder(
        {
          'clean.ts': 'export const retries = 3;\n',
          'dirty.ts': 'export const MAX_RETRIES = 3;\n',
        },
        onTestFinished,
      );

      const clean = consumer.seam([`${folder}/clean.ts`]);
      const whole = consumer.seam([folder]);

      expect(clean.status).toBe(0);
      expect(whole.status).toBe(1);

      expect(linesWith(whole.stdout, 'naming-convention')).toEqual([
        expect.stringContaining('dirty.ts:'),
      ]);
    });

    it('fixes house spacing, repeats the fix unchanged, and formats after a failed fix', async ({
      onTestFinished,
    }) => {
      const { directory, folder } = await caseFolder(
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
      const fix = consumer.seam(['--fix', `${folder}/spacing.ts`]);

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
      const repeated = consumer.seam(['--fix', `${folder}/spacing.ts`]);

      expect(repeated.status).toBe(0);
      expect(await readFile(spacing, 'utf8')).toBe(fixed);

      const manual = consumer.seam(['--fix', `${folder}/manual.ts`]);

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
      onTestFinished,
    }) => {
      const { directory, folder } = await caseFolder(
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

      const check = consumer.seam([folder]);

      expect(check.status).toBe(1);
      expect(linesWith(check.stdout, 'padding-line-between-statements')).toHaveLength(5);

      const fix = consumer.seam(['--fix', folder]);

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

      expect(consumer.seam([folder]).status).toBe(0);
    });

    it('moves types above unrelated values but leaves types derived from a value beside it', async ({
      onTestFinished,
    }) => {
      const { directory, folder } = await caseFolder(
        {
          'home.ts': "export const home = '/home';\n",
          'types.ts': lines(
            "import { home } from './home.ts';",
            '',
            'export const root = home.trim();',
            '',
            '// Describes one request.',
            'export interface Request {',
            '  path: string;',
            '}',
            '',
            'const defaults = { path: root };',
            '',
            'export type Defaults = typeof defaults;',
            '',
            'const build = (path: string): Request => ({ path });',
            '',
            'export type Built = ReturnType<typeof build>;',
            '',
            'type Paths = string[];',
            '',
            'export const paths: Paths = [build(root).path, defaults.path];',
            '',
            'type Name = string; paths.push(root);',
            '',
            'export default interface Options {',
            '  name: Name;',
            '}',
            '',
            'paths.pop(); export type Label = Name;',
          ),
        },
        onTestFinished,
      );

      const check = consumer.seam([folder]);
      const diagnostics = linesWith(check.stdout, 'type-placement');

      expect(check.status).toBe(1);
      expect(diagnostics).toEqual([expect.stringContaining('types.ts:6:')]);

      const fix = consumer.seam(['--fix', folder]);

      expect(fix.status).toBe(0);

      expect(await readFile(join(directory, 'types.ts'), 'utf8')).toBe(
        lines(
          "import { home } from './home.ts';",
          '',
          '// Describes one request.',
          'export interface Request {',
          '  path: string;',
          '}',
          '',
          'type Paths = string[];',
          '',
          'type Name = string;',
          '',
          'export default interface Options {',
          '  name: Name;',
          '}',
          '',
          'export type Label = Name;',
          '',
          'export const root = home.trim();',
          '',
          'const defaults = { path: root };',
          '',
          'export type Defaults = typeof defaults;',
          '',
          'const build = (path: string): Request => ({ path });',
          '',
          'export type Built = ReturnType<typeof build>;',
          '',
          'export const paths: Paths = [build(root).path, defaults.path];',
          '',
          'paths.push(root);',
          '',
          'paths.pop();',
        ),
      );
    });

    it('keeps code inside the lines a moved type comment spans', async ({ onTestFinished }) => {
      const { directory, folder } = await caseFolder(
        {
          'comment.ts': lines(
            'export const count = 1;',
            '',
            'type Label = string; /* This comment',
            '   continues on a second line. */',
            '',
            "export const label: Label = 'ready';",
          ),
        },
        onTestFinished,
      );

      const fix = consumer.seam(['--fix', folder]);

      expect(fix.error).toBeUndefined();
      expect(fix.status).toBe(0);

      expect(await readFile(join(directory, 'comment.ts'), 'utf8')).toBe(
        lines(
          'type Label = string; /* This comment',
          '   continues on a second line. */',
          '',
          'export const count = 1;',
          '',
          "export const label: Label = 'ready';",
        ),
      );
    });

    it('passes a long function, a long file, and five parameters', async ({ onTestFinished }) => {
      const { folder } = await caseFolder(
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

      const result = consumer.seam([folder]);

      expect(diagnosticOutput(result.stdout)).toBe('');
      expect(result.status).toBe(0);
    });

    it('type-checks consumer code against the declarations without skipping libraries', async ({
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

      const { folder } = await caseFolder(
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

      const usage = consumer.typecheck(join(folder, 'tsconfig.usage.json'));
      const misuse = consumer.typecheck(join(folder, 'tsconfig.misuse.json'));

      expect(usage.error).toBeUndefined();
      expect(usage.stdout).toBe('');
      expect(usage.status).toBe(0);
      // A failing misuse proves the declarations resolved to real types, not `any`.
      expect(misuse.status).not.toBe(0);
      expect(misuse.stdout).toContain("Type 'OxlintConfig' is not assignable to type 'number'");
    });

    it('reports each always-on rule added by this package', async ({ onTestFinished }) => {
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

      const { folder } = await caseFolder(files, onTestFinished);
      const result = consumer.seam([folder]);

      expect(result.status).toBe(1);

      for (const [name, [rule]] of Object.entries(fixtures)) {
        const diagnostics = linesWith(result.stdout, `${name}:`);

        expect(diagnostics, name).toEqual(expect.arrayContaining([expect.stringContaining(rule)]));
      }
    });

    it('runs a local plugin of the consumer beside the house rules', async ({ onTestFinished }) => {
      const project = join(consumer.directory, 'local');

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

      const ordinary = consumer.ordinaryLint(['cases'], project);
      const style = consumer.seam(['cases'], project);

      expect(ordinary.error).toBeUndefined();
      expect(ordinary.status).toBe(1);
      expect(ordinary.stdout).toContain('local(no-forbidden-name)');
      expect(ordinary.stdout).not.toContain('seam(');
      expect(style.error).toBeUndefined();
      expect(style.status).toBe(1);
      expect(style.stdout).toContain('local(no-forbidden-name)');
      expect(style.stdout).toContain('seam(naming-convention)');
    });

    it('loads the React and Vitest presets through extends', async ({ onTestFinished }) => {
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

      const { folder } = await caseFolder(files, onTestFinished, 'presets');
      const withPresets = consumer.seam([folder], join(consumer.directory, 'presets'));
      const base = await caseFolder(files, onTestFinished);
      const withoutPresets = consumer.seam([base.folder]);

      expect(withPresets.stdout).toContain('(rules-of-hooks)');
      expect(withPresets.stdout).toContain('(no-focused-tests)');
      expect(withPresets.stdout).toContain('(prefer-import-in-mock)');
      expect(withoutPresets.stdout).not.toContain('rules-of-hooks');
      expect(withoutPresets.stdout).not.toContain('no-focused-tests');
      expect(withoutPresets.stdout).not.toContain('prefer-import-in-mock');
    });
  },
  spawnedTestTimeout,
);
