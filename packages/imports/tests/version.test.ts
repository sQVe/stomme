import { readFile } from 'node:fs/promises';

import { parse } from 'smol-toml';
import { expect, it } from 'vitest';

interface Manifest {
  version?: string;
}

const readJson = async (path: string): Promise<Manifest> =>
  JSON.parse(await readFile(new URL(path, import.meta.url), 'utf8')) as Manifest;

const readToml = async (path: string) =>
  parse(await readFile(new URL(path, import.meta.url), 'utf8'));

const isTable = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

it('matches the version in crates/stomme-cli/Cargo.toml', async () => {
  const own = await readJson('../package.json');
  const cargo = await readToml('../../../crates/stomme-cli/Cargo.toml');
  const packageTable = cargo.package;

  if (!isTable(packageTable)) {
    throw new TypeError('Cargo.toml has no package table');
  }

  expect(own.version).toBeDefined();
  expect(own.version).toBe(packageTable.version);
});
