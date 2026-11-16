import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';

import { afterAll, beforeAll, describe, inject, it } from 'vitest';

import type { Consumer, Runtime } from './consumer.ts';
import { installConsumer, writeFiles } from './consumer.ts';

type CaseFolder = (
  files: Record<string, string>,
  onTestFinished: (cleanup: () => Promise<void>) => void,
) => Promise<{ folder: string; directory: string }>;

export interface PackageContext {
  readonly consumer: Consumer;
  caseFolder: CaseFolder;
}

export type PackageTests = (context: PackageContext) => void;

const configHeader = 'schema = 1\nversion = "0.0.0"\nlanguage = "typescript"\n';

const projectFiles = (config: string, files: Record<string, string>) => ({
  'package.json': JSON.stringify({ name: 'case', private: true, type: 'module' }),
  'stomme.toml': `${configHeader}\n${config}`,
  ...files,
});

const linesWith = (output: string, text: string) =>
  output.split('\n').filter((line) => line.includes(text));

const diagnostics = (output: string) => linesWith(output, 'stomme-imports(module-boundaries)');

const spawnedTestTimeout = 120_000;

// These tests load the package's plugin, so they run under every runtime.
export const loadingTests: PackageTests = (context) => {
  it('loads the plugin by package name and reports a refused import', async ({
    expect,
    onTestFinished,
  }) => {
    const { folder } = await context.caseFolder(
      projectFiles(
        `[modules.alpha]
entry = "src/alpha.ts"

[modules.beta]
entry = "src/beta.ts"
`,
        {
          'src/alpha.ts': "import { beta } from './beta.js';\n\nexport const value = beta;\n",
          'src/beta.ts': 'export const beta = 1;\n',
        },
      ),
      onTestFinished,
    );

    const result = await context.consumer.lint([folder]);

    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(diagnostics(result.stdout)).toHaveLength(1);
    expect(diagnostics(result.stdout)[0]).toContain('must not import module "beta"');
  });
};

// These tests check lint results, which do not depend on the runtime that loads the package.
export const behaviorTests: PackageTests = (context) => {
  it('fails new shared files and shared imports that the module table does not list', async ({
    expect,
    onTestFinished,
  }) => {
    const { folder } = await context.caseFolder(
      projectFiles(
        `[modules.errors]
entry = "src/errors.ts"

[modules.schema]
entry = "src/schema.ts"

[modules.skillTools]
entry = "src/skillTools.ts"

[modules.github]
entry = "src/github.ts"
`,
        {
          'src/example.ts': 'export const value = 1;\n',
          'src/skillTools.ts':
            "import { runGithub } from './github.js';\n\nexport const value = runGithub;\n",
          'src/errors.ts':
            "import type { Problem } from './schema.js';\n\nexport type Value = Problem;\n",
        },
      ),
      onTestFinished,
    );

    const result = await context.consumer.lint([folder]);
    const found = diagnostics(result.stdout);
    const linesFor = (file: string) => found.filter((line) => line.includes(file));

    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(found).toHaveLength(3);
    expect(linesFor('example.ts')[0]).toContain('File "example.ts" is not in a module');
    expect(linesFor('skillTools.ts')[0]).toContain('"skillTools" must not import module "github"');
    expect(linesFor('errors.ts')[0]).toContain('"errors" must not import module "schema"');
  });

  it('keeps extensions out of shared modules but lets the package entry load them', async ({
    expect,
    onTestFinished,
  }) => {
    const { folder } = await context.caseFolder(
      projectFiles(
        `[modules.skillTools]
entry = "src/skillTools.ts"

[modules."extensions/commit"]
entry = "src/extensions/commit/commit.ts"

[modules.tau]
entry = "src/tau.ts"
imports = ["extensions/commit"]
`,
        {
          'src/skillTools.ts':
            "import commit from './extensions/commit/commit.js';\n\nexport default commit;\n",
          'src/tau.ts':
            "import commit from './extensions/commit/commit.js';\n\nexport default commit;\n",
        },
      ),
      onTestFinished,
    );

    const result = await context.consumer.lint([folder]);
    const found = diagnostics(result.stdout);

    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(found).toHaveLength(1);
    expect(found[0]).toContain('skillTools.ts');
  });

  it('keeps private controller files out of the rest of the subagents extension', async ({
    expect,
    onTestFinished,
  }) => {
    const { folder } = await context.caseFolder(
      projectFiles(
        `[modules."extensions/subagents"]
entry = "src/extensions/subagents/subagents.ts"
imports = ["extensions/subagents/controller"]

[modules."extensions/subagents/controller"]
entry = "src/extensions/subagents/controller/controller.ts"
public = ["src/extensions/subagents/controller/budget.ts", "src/extensions/subagents/controller/controller.ts", "src/extensions/subagents/controller/record.ts"]
`,
        {
          'src/extensions/subagents/probe.ts': [
            "import { launchTiming } from './controller/budget.js';",
            "import { WorkerController } from './controller/controller.js';",
            "import { EvidenceUnavailableError } from './controller/record.js';",
            "import { stopOwnedWorker } from './controller/stop.js';",
            '',
            'export const value = [launchTiming, WorkerController, EvidenceUnavailableError, stopOwnedWorker];',
            '',
          ].join('\n'),
        },
      ),
      onTestFinished,
    );

    const result = await context.consumer.lint([folder]);
    const found = diagnostics(result.stdout);

    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(found).toHaveLength(1);
    expect(found[0]).toContain('probe.ts:4:1');
    expect(found[0]).toContain('private file "stop"');
  });

  it('ignores an unrelated src folder above the project root', async ({
    expect,
    onTestFinished,
  }) => {
    const { folder } = await context.caseFolder(
      {
        'src/project/package.json': '{}\n',
        'src/project/scripts/probe.ts': 'export const value = 1;\n',
        'src/project/lint/probe.ts': 'export const value = 1;\n',
        'src/project/tests/probe.ts': 'export const value = 1;\n',
      },
      onTestFinished,
    );

    const result = await context.consumer.lint([folder]);

    expect(result.error).toBeUndefined();
    expect(diagnostics(result.stdout)).toEqual([]);
  });
};

export const describePackage = (runtime: Runtime, groups: PackageTests[]): void => {
  describe.concurrent(
    `the built package under ${runtime.name}`,
    () => {
      let consumerDirectory = '';
      let consumer: Consumer;

      // Each test writes its fixtures into its own folder inside the consumer project.
      const caseFolder: CaseFolder = async (files, onTestFinished) => {
        const cases = join(consumer.directory, 'cases');

        await mkdir(cases, { recursive: true });

        const directory = await mkdtemp(join(cases, 'case-'));
        const folder = relative(consumer.directory, directory);

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
        consumerDirectory = await mkdtemp(join(tmpdir(), 'stomme-imports-consumer-'));
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
