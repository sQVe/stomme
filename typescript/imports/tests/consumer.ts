import { spawn } from 'node:child_process';
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export interface Runtime {
  name: string;
  executable: string;
  install: string[];
}

export interface CommandResult {
  status: number | null;
  stdout: string;
  stderr: string;
  error: Error | undefined;
}

export interface Consumer {
  directory: string;
  lint: (argumentsList: string[], cwd?: string) => Promise<CommandResult>;
}

const root = fileURLToPath(new URL('../', import.meta.url));
const commandTimeout = 120_000;

// pnpm installs a strict, non-hoisted tree, so the package must resolve its own dependencies.
export const nodeRuntime: Runtime = {
  name: 'Node with pnpm',
  executable: process.execPath,
  install: ['pnpm', 'install'],
};

export const bunRuntime: Runtime = { name: 'Bun', executable: 'bun', install: ['bun', 'install'] };

const run = (
  command: string,
  argumentsList: string[],
  cwd: string,
  environment = process.env,
): Promise<CommandResult> =>
  new Promise((resolve) => {
    const child = spawn(command, argumentsList, {
      cwd,
      env: environment,
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    let stdout = '';
    let stderr = '';
    let error: Error | undefined;

    // Node's own spawn timeout keeps its timer alive after a spawn error, so this one is cleared.
    const timer = setTimeout(() => {
      error = new Error(`${command} timed out after ${commandTimeout} ms`);
      child.kill();
    }, commandTimeout);

    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');

    child.stdout.on('data', (text: string) => {
      stdout += text;
    });

    child.stderr.on('data', (text: string) => {
      stderr += text;
    });

    child.on('error', (spawnError) => {
      clearTimeout(timer);
      error ??= spawnError;
    });

    child.on('close', (status, signal) => {
      clearTimeout(timer);

      const stopped = signal === null ? undefined : new Error(`${command} stopped by ${signal}`);

      resolve({ status, stdout, stderr, error: error ?? stopped });
    });
  });

const succeed = async (command: string, argumentsList: string[], cwd: string) => {
  const result = await run(command, argumentsList, cwd);

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

// Oxlint prints one line per diagnostic only when it detects an agent; otherwise the format
// depends on the shell that runs the tests.
const agentEnvironment = () => ({ ...process.env, AI_AGENT: 'stomme-imports-tests', CI: '1' });

export const packPackage = async (destination: string): Promise<string> => {
  await succeed('pnpm', ['pack', '--pack-destination', destination], root);

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
  await writeFiles(directory, consumerFiles(tarball));

  const [installer = 'pnpm', ...installArguments] = runtime.install;

  await succeed(installer, installArguments, directory);

  const vitePlus = join(directory, 'node_modules/vite-plus/dist/bin.js');

  return {
    directory,
    lint: (argumentsList, cwd = directory) =>
      run(
        runtime.executable,
        [vitePlus, 'lint', '--deny-warnings', ...argumentsList],
        cwd,
        agentEnvironment(),
      ),
  };
};
