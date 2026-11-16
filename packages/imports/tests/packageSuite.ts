import { it } from 'vitest';

import type { Runtime } from '../../style/tests/consumer.ts';
import type { PackageTests } from '../../style/tests/packageHarness.ts';
import { describePackage as describeSharedPackage } from '../../style/tests/packageHarness.ts';
import type { Consumer } from './consumer.ts';
import { installConsumer } from './consumer.ts';
import { packageVersion } from './packageVersion.ts';

const configHeader = `schema = 1\nversion = "${packageVersion}"\nlanguage = "typescript"\n`;

const projectFiles = (config: string, files: Record<string, string>) => ({
  'package.json': JSON.stringify({ name: 'case', private: true, type: 'module' }),
  'stomme.toml': `${configHeader}\n${config}`,
  ...files,
});

const linesWith = (output: string, text: string) =>
  output.split('\n').filter((line) => line.includes(text));

const diagnostics = (output: string) => linesWith(output, 'stomme-imports(module-boundaries)');

export const loadingTests: PackageTests<Consumer> = (context) => {
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

export const behaviorTests: PackageTests<Consumer> = (context) => {
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

// Each runtime's test file installs its own consumer, and its tests run at the same time.
export const describePackage = (runtime: Runtime, groups: PackageTests<Consumer>[]): void => {
  describeSharedPackage(runtime, groups, {
    consumerPrefix: 'stomme-imports-consumer-',
    installConsumer,
  });
};
