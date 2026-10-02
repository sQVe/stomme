import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export interface ChildResult {
  status: number | null;
  error?: Error | undefined;
}

export type RunVitePlus = (commandArguments: string[], styleEnabled: boolean) => ChildResult;

export const runVitePlus: RunVitePlus = (commandArguments, styleEnabled) => {
  // Vite Plus is a peer dependency, so this resolves the consumer's copy, never a second one.
  const executable = fileURLToPath(import.meta.resolve('vite-plus/bin'));

  // The current runtime runs the child, so Bun stays Bun and Node stays Node.
  return spawnSync(process.execPath, [executable, ...commandArguments], {
    stdio: 'inherit',
    // eslint-disable-next-line node/no-process-env -- Only the lint child enables house style.
    env: { ...process.env, SEAM_STYLE: styleEnabled ? '1' : '0' },
  });
};

const statusOf = (result: ChildResult): number => {
  if (result.error !== undefined) {
    throw result.error;
  }

  return result.status ?? 1;
};

export const runSeam = (argumentsList: readonly string[], run: RunVitePlus): number => {
  const fixing = argumentsList[0] === '--fix';
  const paths = fixing ? argumentsList.slice(1) : argumentsList;
  const lintArguments = ['lint', '--deny-warnings'];

  if (fixing) {
    lintArguments.push('--fix');
  }

  const lintStatus = statusOf(run([...lintArguments, ...paths], true));
  // Format even when a manual rename or helper move is still needed.
  const formatStatus = fixing ? statusOf(run(['fmt', ...paths], false)) : 0;

  return lintStatus || formatStatus;
};
