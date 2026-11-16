import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import type { TestProject } from 'vitest/node';

import { packPackage } from './consumer.ts';

declare module 'vitest' {
  export interface ProvidedContext {
    tarball: string;
  }
}

export const createPackSetup =
  (prefix: string, packageDirectory: string) =>
  async (project: TestProject): Promise<() => Promise<void>> => {
    const directory = await mkdtemp(join(tmpdir(), prefix));

    const removeDirectory = () => rm(directory, { recursive: true, force: true });

    try {
      project.provide('tarball', await packPackage(directory, packageDirectory));
    } catch (error) {
      await removeDirectory();

      throw error;
    }

    return removeDirectory;
  };

const stylePackageDirectory = fileURLToPath(new URL('../', import.meta.url));

// Packs once per run so every runtime's test file installs the same tarball.
export default createPackSetup('seam-pack-', stylePackageDirectory);
