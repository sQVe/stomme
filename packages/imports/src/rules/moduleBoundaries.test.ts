import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterAll } from 'vitest';

import { packageVersion } from '../../tests/packageVersion.ts';
import { ruleTester } from '../../tests/ruleTester.ts';
import { tauModuleConfig } from '../../tests/tauModuleConfig.ts';
import { moduleBoundariesRule } from './moduleBoundaries.ts';

const temporaryRoots: string[] = [];

const projectRoot = (config: string | undefined): string => {
  const root = mkdtempSync(join(tmpdir(), 'stomme-imports-'));

  temporaryRoots.push(root);
  writeFileSync(join(root, 'package.json'), '{ "name": "fixture", "private": true }\n');

  if (config !== undefined) {
    writeFileSync(join(root, 'stomme.toml'), config);
  }

  return root;
};

const configHeader = `schema = 1\nversion = "${packageVersion}"\nlanguage = "typescript"\n`;

afterAll(() => {
  for (const root of temporaryRoots) {
    rmSync(root, { recursive: true, force: true });
  }
});

const crossingRoot = projectRoot(
  `${configHeader}
[modules.alpha]
entry = "src/alpha.ts"

[modules.beta]
entry = "src/beta.ts"
imports = ["alpha"]
`,
);

const folderRoot = projectRoot(
  `${configHeader}
[modules.config]
entry = "src/config/config.ts"
public = ["src/config/models.ts"]

[modules.consumer]
entry = "src/consumer.ts"
imports = ["config"]
`,
);

const commonJsRoot = projectRoot(
  `${configHeader}
[modules.alpha]
entry = "src/alpha/index.cts"

[modules.beta]
entry = "src/beta/index.cts"
`,
);

const tauRoot = projectRoot(tauModuleConfig);
const noConfigRoot = projectRoot(undefined);
const brokenRoot = projectRoot(configHeader.replace('typescript', 'rust'));

const tauFile = (path: string) => join(tauRoot, path);

ruleTester.run('module-boundaries', moduleBoundariesRule, {
  valid: [
    {
      name: 'allows an import that the table lists',
      code: "import { alpha } from './alpha';\n\nexport const value = alpha;",
      filename: join(crossingRoot, 'src/beta.ts'),
    },
    {
      name: 'ignores a target in no module',
      code: "import { value } from './unlisted.js';\n\nexport const result = value;",
      filename: join(crossingRoot, 'src/alpha.ts'),
    },
    {
      name: 'ignores a target outside the source root',
      code: "import { value } from '../outside.js';\n\nexport const result = value;",
      filename: join(crossingRoot, 'src/alpha.ts'),
    },
    {
      name: 'ignores a file outside the source root without a config',
      code: 'export const value = 1;',
      filename: join(noConfigRoot, 'tests/probe.ts'),
    },
    {
      name: 'allows a public file of an owner folder',
      code: "import { models } from './config/models.js';\n\nexport const value = models;",
      filename: join(folderRoot, 'src/consumer.ts'),
    },
    {
      name: 'allows an extensionless folder import',
      code: "import { config } from './config';\n\nexport const value = config;",
      filename: join(folderRoot, 'src/consumer.ts'),
    },
    {
      name: 'allows the entry file of an owner folder',
      code: "import { config } from './config/config.js';\n\nexport const value = config;",
      filename: join(folderRoot, 'src/consumer.ts'),
    },
    {
      code: "import { own } from './own.js';\n\nexport const value = own;",
      filename: tauFile('src/extensions/probe/probe.ts'),
    },
    {
      code: "import { errorMessage } from '../../errors.js';\n\nexport const value = errorMessage;",
      filename: tauFile('src/extensions/probe/probe.ts'),
    },
    {
      code: "import { parseModelEntry } from '../../config/models.js';\n\nexport const value = parseModelEntry;",
      filename: tauFile('src/extensions/probe/probe.ts'),
    },
    {
      code: "import { errorMessage } from '../errors.js';\n\nexport const value = errorMessage;",
      filename: tauFile('src/extensions/flat.ts'),
    },
    {
      code: "import commit from './extensions/commit/commit.js';\nimport coding from './extensions/coding.js';\n\nexport default [commit, coding];",
      filename: tauFile('src/tau.ts'),
    },
    {
      code: "import { readOwnedWorker } from './extensions/subagents/controller/record.js';\n\nexport const value = readOwnedWorker;",
      filename: tauFile('src/tau.ts'),
    },
    {
      code: "import { readOwnedWorker } from './controller/record.js';\nimport { names } from './names.js';\n\nexport const value = [readOwnedWorker, names];",
      filename: tauFile('src/extensions/subagents/subagents.ts'),
    },
    {
      code: "import { effectiveAllowedModels } from '../../config/allowedModels.js';\n\nexport const value = effectiveAllowedModels;",
      filename: tauFile('src/extensions/probe/probe.test.ts'),
    },
    {
      code: "import { effectiveAllowedModels } from '../../../config/allowedModels.js';\n\nexport const value = effectiveAllowedModels;",
      filename: tauFile('src/extensions/probe/fixtures/fake.ts'),
    },
    {
      code: "import { helper } from '../src/extensions/probe/helper.js';\n\nexport const value = helper;",
      filename: tauFile('tests/probe.ts'),
    },
    {
      code: "import { describeSchemaProblem } from './schema.js';\n\nexport const value = describeSchemaProblem;",
      filename: tauFile('src/command.ts'),
    },
    {
      code: "import { withFileLock } from '../fileLock/fileLock.js';\n\nexport const value = withFileLock;",
      filename: tauFile('src/inbox/endpoint.ts'),
    },
    {
      code: "import { startInbox } from '../../inbox/inbox.js';\n\nexport const value = startInbox;",
      filename: tauFile('src/extensions/inbox/lifecycle.ts'),
    },
    {
      code: "import { parseModelEntry } from './models.js';\nimport { readConfig } from './config.js';\n\nexport const value = [parseModelEntry, readConfig];",
      filename: tauFile('src/config/allowedModels.ts'),
    },
    {
      code: "import type { Problem } from './schema.js';\n\nexport type Value = Problem;",
      filename: tauFile('src/command.ts'),
    },
    {
      code: "import { keys } from '../skillTools.js';\nexport const value = keys;",
      filename: tauFile('src/reviewCapture/fixtures/fakeGit.ts'),
    },
    {
      code: "import { anything } from './github.js';\n\nexport const value = anything;",
      filename: tauFile('src/skillTools.test.ts'),
    },
    {
      code: 'export const load = (name: string) => import(name);',
      filename: tauFile('src/skillTools.test.ts'),
    },
    {
      name: 'allows a flat support file outside any module to import a shared module',
      code: "import { schema } from './schema.js';\n\nexport const value = schema;",
      filename: tauFile('src/alpha.test.ts'),
    },
    {
      code: 'export const value = 1;',
      filename: tauFile('src/skills/tdd/helper.ts'),
    },
    {
      code: 'export const value = 1;',
      filename: tauFile('src/extensions/snippets/snippets.ts'),
    },
    {
      code: 'export const load = (name: string) => import(name);',
      filename: tauFile('tests/probe.ts'),
    },
    {
      code: "import { stopOwnedWorker } from './stop.js';\nimport { names } from '../names.js';\n\nexport const value = [stopOwnedWorker, names];",
      filename: tauFile('src/extensions/subagents/controller/controller.ts'),
    },
    {
      code: "import { readOwnedWorker } from './controller/record.js';\n\nexport const value = readOwnedWorker;",
      filename: tauFile('src/extensions/subagents/subagents.ts'),
    },
    {
      code: "import { lookupRunner } from './runner/runners.js';\n\nexport const value = lookupRunner;",
      filename: tauFile('src/extensions/tdd/observation.ts'),
    },
    {
      code: "import { snippet } from './asset.js';\n\nexport const value = snippet;",
      filename: tauFile('src/extensions/snippets/snippets/loader.ts'),
    },
    {
      code: "import { keys } from '../../../skillTools.js';\n\nexport const value = keys;",
      filename: tauFile('src/extensions/snippets/fixtures/helper.ts'),
    },
  ],
  invalid: [
    {
      name: 'reports an import that crosses the table, naming the edge',
      code: "import { beta } from './beta.js';\n\nexport const value = beta;",
      filename: join(crossingRoot, 'src/alpha.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'alpha', target: 'beta' } }],
    },
    {
      name: 'reports a type import that crosses the table, naming the edge',
      code: "import type { Beta } from './beta.js';\n\nexport type Value = Beta;",
      filename: join(crossingRoot, 'src/alpha.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'alpha', target: 'beta' } }],
    },
    {
      name: 'reports a forbidden import through a CommonJS specifier',
      code: "import { beta } from '../beta/index.cjs';\n\nexport const value = beta;",
      filename: join(commonJsRoot, 'src/alpha/index.cts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'alpha', target: 'beta' } }],
    },
    {
      name: 'reports a type import expression that crosses the table',
      code: "export type Value = import('./beta.js').Beta;",
      filename: join(crossingRoot, 'src/alpha.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'alpha', target: 'beta' } }],
    },
    {
      name: 'reports a template literal import that crosses the table',
      code: 'export const load = () => import(`./beta.js`);',
      filename: join(crossingRoot, 'src/alpha.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'alpha', target: 'beta' } }],
    },
    {
      name: 'reports a flat support file outside any module that imports the root',
      code: "import tau from './tau.js';\n\nexport default tau;",
      filename: tauFile('src/alpha.test.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'alpha.test', target: 'tau' } }],
    },
    {
      name: 'reports a reach into an owner folder past its entry file',
      code: "import { allowed } from './config/allowed.js';\n\nexport const value = allowed;",
      filename: join(folderRoot, 'src/consumer.ts'),
      errors: [{ messageId: 'private', data: { folder: 'config', file: 'allowed' } }],
    },
    {
      name: 'reports a refused stomme.toml once',
      code: 'export const value = 1;',
      filename: join(brokenRoot, 'src/example.ts'),
      errors: [
        {
          messageId: 'brokenConfig',
          data: {
            reason: 'has `language = "rust"`; this package reads only `language = "typescript"`',
          },
        },
      ],
    },
    {
      name: 'reports a missing stomme.toml once',
      code: 'export const value = 1;',
      filename: join(noConfigRoot, 'src/example.ts'),
      errors: [{ messageId: 'noConfig' }],
    },
    {
      code: "import coding from '../coding.js';\n\nexport default coding;",
      filename: tauFile('src/extensions/probe/probe.test.ts'),
      errors: [
        {
          messageId: 'notAllowed',
          data: { source: 'extensions/probe', target: 'extensions/coding' },
        },
      ],
    },
    {
      code: "import coding from '../../coding.js';\n\nexport default coding;",
      filename: tauFile('src/extensions/probe/fixtures/fake.ts'),
      errors: [
        {
          messageId: 'notAllowed',
          data: { source: 'extensions/probe', target: 'extensions/coding' },
        },
      ],
    },
    {
      code: "import tau from '../../tau.js';\n\nexport default tau;",
      filename: tauFile('src/extensions/probe/probe.test.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'extensions/probe', target: 'tau' } }],
    },
    {
      code: "import { stopOwnedWorker } from './controller/stop.js';\n\nexport const value = stopOwnedWorker;",
      filename: tauFile('src/extensions/subagents/subagents.ts'),
      errors: [
        {
          messageId: 'private',
          data: { folder: 'extensions/subagents/controller', file: 'stop' },
        },
      ],
    },
    {
      code: "import { defaultSpawn } from './runner/process.js';\n\nexport const value = defaultSpawn;",
      filename: tauFile('src/extensions/tdd/observation.ts'),
      errors: [
        { messageId: 'private', data: { folder: 'extensions/tdd/runner', file: 'process' } },
      ],
    },
    {
      code: "import coding from '../coding.js';\n\nexport default coding;",
      filename: tauFile('src/extensions/snippets/snippets.ts'),
      errors: [
        {
          messageId: 'notAllowed',
          data: { source: 'extensions/snippets', target: 'extensions/coding' },
        },
      ],
    },
    {
      code: "import { commitToolGuidelines } from '../commit/tool.js';\n\nexport const value = commitToolGuidelines;",
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [
        { messageId: 'private', data: { folder: 'extensions/commit', file: 'tool' } },
        {
          messageId: 'notAllowed',
          data: { source: 'extensions/probe', target: 'extensions/commit' },
        },
      ],
    },
    {
      code: "export { commitToolGuidelines } from '../commit/tool.js';",
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [{ messageId: 'private' }, { messageId: 'notAllowed' }],
    },
    {
      code: "export const load = () => import('../commit/tool.js');",
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [{ messageId: 'private' }, { messageId: 'notAllowed' }],
    },
    {
      code: "import coding from '../coding.js';\n\nexport default coding;",
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [
        {
          messageId: 'notAllowed',
          data: { source: 'extensions/probe', target: 'extensions/coding' },
        },
      ],
    },
    {
      code: "import commit from './commit/commit.js';\n\nexport default commit;",
      filename: tauFile('src/extensions/flat.ts'),
      errors: [
        {
          messageId: 'notAllowed',
          data: { source: 'extensions/flat', target: 'extensions/commit' },
        },
      ],
    },
    {
      code: "import tau from '../../tau.js';\n\nexport default tau;",
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'extensions/probe', target: 'tau' } }],
    },
    {
      code: "import tau from '../tau.js';\n\nexport default tau;",
      filename: tauFile('src/extensions/flat.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'extensions/flat', target: 'tau' } }],
    },
    {
      code: "import tau from './tau.js';\n\nexport default tau;",
      filename: tauFile('src/skillTools.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'skillTools', target: 'tau' } }],
    },
    {
      code: "import tau from '../tau.js';\n\nexport default tau;",
      filename: tauFile('src/config/models.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'config', target: 'tau' } }],
    },
    {
      code: "import { effectiveAllowedModels } from '../../config/allowedModels.js';\n\nexport const value = effectiveAllowedModels;",
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [{ messageId: 'private', data: { folder: 'config', file: 'allowedModels' } }],
    },
    {
      code: "import { effectiveAllowedModels } from './config/allowedModels.js';\n\nexport const value = effectiveAllowedModels;",
      filename: tauFile('src/tau.ts'),
      errors: [{ messageId: 'private', data: { folder: 'config', file: 'allowedModels' } }],
    },
    {
      code: 'export const value = 1;',
      filename: tauFile('src/example.ts'),
      errors: [{ messageId: 'unknownFile', data: { file: 'example.ts' } }],
    },
    {
      code: 'export const value = 1;',
      filename: tauFile('src/unlisted/helper.ts'),
      errors: [{ messageId: 'unknownFile', data: { file: 'unlisted/helper.ts' } }],
    },
    {
      code: "import { keys } from './skillTools.js';\n\nexport const value = keys;",
      filename: tauFile('src/command.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'command', target: 'skillTools' } }],
    },
    {
      code: "import type { Keys } from './skillTools.js';\n\nexport type Value = Keys;",
      filename: tauFile('src/command.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'command', target: 'skillTools' } }],
    },
    {
      code: "export { keys } from './skillTools.js';",
      filename: tauFile('src/command.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'command', target: 'skillTools' } }],
    },
    {
      code: "export const load = () => import('./skillTools.js');",
      filename: tauFile('src/command.ts'),
      errors: [{ messageId: 'notAllowed' }],
    },
    {
      code: "import { getAgentDirectory } from '../tauDirectory.js';\n\nexport const value = getAgentDirectory;",
      filename: tauFile('src/inbox/endpoint.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'inbox', target: 'tauDirectory' } }],
    },
    {
      code: "import { openEndpoint } from '../../inbox/endpoint.js';\n\nexport const value = openEndpoint;",
      filename: tauFile('src/extensions/inbox/lifecycle.ts'),
      errors: [{ messageId: 'private', data: { folder: 'inbox', file: 'endpoint' } }],
    },
    {
      code: "import { keys } from '../skillTools.js';\n\nexport const value = keys;",
      filename: tauFile('src/config/models.ts'),
      errors: [{ messageId: 'notAllowed', data: { source: 'config', target: 'skillTools' } }],
    },
    {
      code: "import { commitToolGuidelines } from './extensions/commit/commit.js';\n\nexport const value = commitToolGuidelines;",
      filename: tauFile('src/skillTools.ts'),
      errors: [
        { messageId: 'notAllowed', data: { source: 'skillTools', target: 'extensions/commit' } },
      ],
    },
    {
      code: 'export const load = (name: string) => import(name);',
      filename: tauFile('src/skillTools.ts'),
      errors: [{ messageId: 'computedImport' }],
    },
    {
      code: 'export const load = (name: string) => import(name);',
      filename: tauFile('src/extensions/probe/probe.ts'),
      errors: [{ messageId: 'computedImport' }],
    },
  ],
});
