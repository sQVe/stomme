import { readFile } from 'node:fs/promises';

interface Manifest {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

const readManifest = async (url: URL): Promise<Manifest> =>
  JSON.parse(await readFile(url, 'utf8')) as Manifest;

// Vite Plus declares exact versions as `=1.2.3`.
const exactVersion = (version: string | undefined) => version?.replace(/^=/, '');

const pinnedNames = ['oxlint', 'vitest', 'oxlint-tsgolint', '@oxlint/plugins'];

export const pinMismatches = async (packageDirectory: URL): Promise<string[]> => {
  const own = await readManifest(new URL('package.json', packageDirectory));

  const vitePlus = await readManifest(
    new URL('node_modules/vite-plus/package.json', packageDirectory),
  );

  const ownVersions = { ...own.dependencies, ...own.devDependencies };
  const mismatches: string[] = [];

  for (const name of pinnedNames) {
    const ownVersion = ownVersions[name];
    const expectedVersion = exactVersion(vitePlus.dependencies?.[name]);

    if (ownVersion === undefined) {
      mismatches.push(`${name} is not pinned`);
    } else if (ownVersion !== expectedVersion) {
      mismatches.push(`${name} is ${ownVersion}, expected ${expectedVersion}`);
    }
  }

  return mismatches;
};
