import { spawnSync } from 'node:child_process';
import type { SpawnSyncReturns } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readdir, realpath, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export interface Runtime {
  name: string;
  executable: string;
  install: string[];
}

export interface Consumer {
  directory: string;
  stickler: (argumentsList: string[], cwd?: string) => SpawnSyncReturns<string>;
  ordinaryLint: (argumentsList: string[], cwd?: string) => SpawnSyncReturns<string>;
  typecheck: (project: string) => SpawnSyncReturns<string>;
}

const root = fileURLToPath(new URL('../', import.meta.url));
const commandTimeout = 120_000;
// The consumer fixtures install no compiler, so type checks use this repository's TypeScript.
const typescriptCompiler = join(root, 'node_modules/typescript/bin/tsc');

// pnpm installs a strict, non-hoisted tree, so the package must resolve its own dependencies.
export const runtimes: Runtime[] = [
  { name: 'Node with pnpm', executable: process.execPath, install: ['pnpm', 'install'] },
  { name: 'Bun', executable: 'bun', install: ['bun', 'install'] },
];

const run = (command: string, argumentsList: string[], cwd: string, environment = process.env) =>
  spawnSync(command, argumentsList, {
    cwd,
    encoding: 'utf8',
    env: environment,
    timeout: commandTimeout,
  });

const succeed = (command: string, argumentsList: string[], cwd: string) => {
  const result = run(command, argumentsList, cwd);

  if (result.error !== undefined) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(
      `${command} ${argumentsList.join(' ')} failed:\n${result.stdout}${result.stderr}`,
    );
  }

  return result.stdout;
};

// The style switch must come from the runner alone, never from the test's own environment.
const environmentWithoutStyle = () => {
  const { STICKLER_STYLE: _styleSwitch, ...environment } = process.env;

  return { ...environment, CI: '1' };
};

export const packPackage = async (destination: string): Promise<string> => {
  succeed('pnpm', ['pack', '--pack-destination', destination], root);

  const tarball = (await readdir(destination)).find((name) => name.endsWith('.tgz'));

  if (tarball === undefined) {
    throw new Error(`pnpm pack wrote no tarball to ${destination}`);
  }

  return join(destination, tarball);
};

export const writeFiles = async (
  directory: string,
  files: Record<string, string>,
): Promise<void> => {
  for (const [name, text] of Object.entries(files)) {
    const path = join(directory, name);

    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, text);
  }
};

const consumerFiles = (tarball: string) => ({
  'package.json': JSON.stringify({
    name: 'consumer',
    private: true,
    type: 'module',
    devDependencies: { '@sqve/stickler': `file:${tarball}`, 'vite-plus': '0.3.0' },
  }),
  'tsconfig.json': JSON.stringify({
    compilerOptions: {
      strict: true,
      module: 'NodeNext',
      target: 'ES2024',
      jsx: 'react-jsx',
      noEmit: true,
      skipLibCheck: true,
    },
    include: ['**/*.ts', '**/*.tsx'],
  }),
  'vite.config.ts': [
    "import { format, lint } from '@sqve/stickler';",
    "import { defineConfig } from 'vite-plus';",
    '',
    'export default defineConfig({ lint, fmt: format });',
    '',
  ].join('\n'),
  // A second project in the same install shows how presets compose with the base config.
  'presets/vite.config.ts': [
    "import { lint, react, vitest } from '@sqve/stickler';",
    "import { defineConfig } from 'vite-plus';",
    '',
    'export default defineConfig({ lint: { extends: [lint, react, vitest] } });',
    '',
  ].join('\n'),
  // Vite Plus reads the config of the nearest package.
  'presets/package.json': JSON.stringify({ name: 'presets', private: true, type: 'module' }),
  'presets/tsconfig.json': JSON.stringify({
    extends: '../tsconfig.json',
    include: ['**/*.ts', '**/*.tsx'],
  }),
});

export const installConsumer = async (
  runtime: Runtime,
  tarball: string,
  directory: string,
): Promise<Consumer> => {
  await writeFiles(directory, consumerFiles(tarball));

  const [installer = 'pnpm', ...installArguments] = runtime.install;

  succeed(installer, installArguments, directory);

  const cli = join(directory, 'node_modules/@sqve/stickler/dist/cli.js');
  const vitePlus = join(directory, 'node_modules/vite-plus/dist/bin.js');

  return {
    directory,
    stickler: (argumentsList, cwd = directory) =>
      run(runtime.executable, [cli, ...argumentsList], cwd, environmentWithoutStyle()),
    ordinaryLint: (argumentsList, cwd = directory) =>
      run(
        runtime.executable,
        [vitePlus, 'lint', '--deny-warnings', ...argumentsList],
        cwd,
        environmentWithoutStyle(),
      ),
    typecheck: (project) => run(process.execPath, [typescriptCompiler, '-p', project], directory),
  };
};

const ancestorsOf = (path: string): string[] => {
  const parent = dirname(path);

  return parent === path ? [path] : [path, ...ancestorsOf(parent)];
};

// Follows Node's lookup: the nearest `node_modules/vite-plus` at or above the package wins.
export const vitePlusSeenByPackage = async (directory: string): Promise<string | undefined> => {
  const packageDirectory = await realpath(join(directory, 'node_modules/@sqve/stickler'));

  for (const ancestor of ancestorsOf(packageDirectory)) {
    const candidate = join(ancestor, 'node_modules/vite-plus');

    if (existsSync(candidate)) {
      return realpath(candidate);
    }
  }

  return undefined;
};
