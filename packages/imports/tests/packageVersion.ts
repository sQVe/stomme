import { readFileSync } from 'node:fs';

const readPackageVersion = (): string => {
  const manifest: unknown = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
  );

  if (typeof manifest !== 'object' || manifest === null || !('version' in manifest)) {
    throw new TypeError('package.json has no version');
  }

  const { version } = manifest;

  if (typeof version !== 'string') {
    throw new TypeError('package.json has no string version');
  }

  return version;
};

export const packageVersion = readPackageVersion();

// Shared fixtures name version 0.0.0 so that a release bump changes no fixture.
export const withPackageVersion = (config: string): string =>
  config.replace('version = "0.0.0"', `version = "${packageVersion}"`);
