import { readFile } from 'node:fs/promises';

import { expect, it } from 'vitest';

interface Manifest {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

const readManifest = async (path: string): Promise<Manifest> =>
  JSON.parse(await readFile(new URL(path, import.meta.url), 'utf8')) as Manifest;

// Vite Plus declares exact versions as `=1.2.3`.
const exactVersion = (version: string | undefined) => version?.replace(/^=/, '');

it('pins the tools Vite Plus depends on to the versions it declares', async () => {
  const own = await readManifest('../package.json');
  const vitePlus = await readManifest('../node_modules/vite-plus/package.json');
  const ownVersions = { ...own.dependencies, ...own.devDependencies };

  for (const name of ['oxlint', 'vitest', 'oxlint-tsgolint', '@oxlint/plugins']) {
    expect(ownVersions[name], name).toBeDefined();
    expect(ownVersions[name], name).toBe(exactVersion(vitePlus.dependencies?.[name]));
  }
});
