import { join } from 'node:path';

import type { CommandResult, Runtime } from '../../style/tests/consumer.ts';
import { agentEnvironment, installPackage, run } from '../../style/tests/consumer.ts';

export { bunRuntime, nodeRuntime } from '../../style/tests/consumer.ts';
export type { CommandResult, Runtime } from '../../style/tests/consumer.ts';

export interface Consumer {
  directory: string;
  lint: (argumentsList: string[], cwd?: string) => Promise<CommandResult>;
}

const consumerFiles = (tarball: string) => ({
  'package.json': JSON.stringify({
    name: 'consumer',
    private: true,
    type: 'module',
    devDependencies: { '@sqve/stomme-ts-imports': `file:${tarball}`, 'vite-plus': '0.3.0' },
  }),
  'tsconfig.json': JSON.stringify({
    compilerOptions: {
      strict: true,
      module: 'NodeNext',
      target: 'ES2024',
      noEmit: true,
      skipLibCheck: true,
    },
    include: ['**/*.ts'],
  }),
  'vite.config.ts': [
    "import { defineConfig } from 'vite-plus';",
    '',
    'export default defineConfig({',
    '  lint: {',
    "    jsPlugins: ['@sqve/stomme-ts-imports'],",
    "    rules: { 'stomme-imports/module-boundaries': 'error' },",
    '  },',
    '});',
    '',
  ].join('\n'),
});

export const installConsumer = async (
  runtime: Runtime,
  tarball: string,
  directory: string,
): Promise<Consumer> => {
  await installPackage(runtime, directory, consumerFiles(tarball));

  const vitePlus = join(directory, 'node_modules/vite-plus/dist/bin.js');

  return {
    directory,
    lint: (argumentsList, cwd = directory) =>
      run(
        runtime.executable,
        [vitePlus, 'lint', '--deny-warnings', ...argumentsList],
        cwd,
        agentEnvironment('stomme-imports-tests'),
      ),
  };
};
