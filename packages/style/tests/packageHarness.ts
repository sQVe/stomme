import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';

import { afterAll, beforeAll, describe, inject } from 'vitest';

import type { Consumer, Runtime } from './consumer.ts';
import { writeFiles } from './consumer.ts';

type CaseFolder = (
  files: Record<string, string>,
  onTestFinished: (cleanup: () => Promise<void>) => void,
  base?: string,
) => Promise<{ folder: string; directory: string }>;

export interface PackageContext<ConsumerType extends { directory: string } = Consumer> {
  readonly consumer: ConsumerType;
  caseFolder: CaseFolder;
}

export type PackageTests<ConsumerType extends { directory: string } = Consumer> = (
  context: PackageContext<ConsumerType>,
) => void;

export interface PackageSuiteOptions<ConsumerType extends { directory: string }> {
  consumerPrefix: string;
  installConsumer: (runtime: Runtime, tarball: string, directory: string) => Promise<ConsumerType>;
}

// Each test spawns up to three commands, and a fix runs lint and the formatter.
const spawnedTestTimeout = 120_000;

// A type-aware run loads every file of the tsconfig project that covers a linted file. A case folder
// is therefore its own project, so a run never reads the folders other tests create and remove.
const caseProject = JSON.stringify({
  extends: '../../tsconfig.json',
  include: ['**/*.ts', '**/*.tsx'],
});

export const describePackage = <ConsumerType extends { directory: string }>(
  runtime: Runtime,
  groups: PackageTests<ConsumerType>[],
  options: PackageSuiteOptions<ConsumerType>,
): void => {
  describe.concurrent(
    `the built package under ${runtime.name}`,
    () => {
      let consumerDirectory = '';
      let consumer: ConsumerType;

      const caseFolder: CaseFolder = async (files, onTestFinished, base = '') => {
        const project = join(consumer.directory, base);
        const cases = join(project, 'cases');

        await mkdir(cases, { recursive: true });

        const directory = await mkdtemp(join(cases, 'case-'));
        const folder = relative(project, directory);

        await writeFiles(directory, { 'tsconfig.json': caseProject, ...files });
        onTestFinished(() => rm(directory, { recursive: true, force: true }));

        return { folder, directory };
      };

      const context: PackageContext<ConsumerType> = {
        get consumer() {
          return consumer;
        },
        caseFolder,
      };

      beforeAll(async () => {
        consumerDirectory = await mkdtemp(join(tmpdir(), options.consumerPrefix));
        consumer = await options.installConsumer(runtime, inject('tarball'), consumerDirectory);
      }, 300_000);

      afterAll(async () => {
        await rm(consumerDirectory, { recursive: true, force: true });
      });

      for (const group of groups) {
        group(context);
      }
    },
    spawnedTestTimeout,
  );
};
