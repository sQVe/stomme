import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import type { TestProject } from 'vitest/node';

import { packPackage } from './consumer.ts';

declare module 'vitest' {
  export interface ProvidedContext {
    tarball: string;
  }
}

export default async function setup(project: TestProject): Promise<() => Promise<void>> {
  const directory = await mkdtemp(join(tmpdir(), 'stomme-imports-pack-'));

  const removeDirectory = () => rm(directory, { recursive: true, force: true });

  try {
    project.provide('tarball', await packPackage(directory));
  } catch (error) {
    await removeDirectory();

    throw error;
  }

  return removeDirectory;
}
