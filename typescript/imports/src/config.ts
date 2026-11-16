import { existsSync, readFileSync, statSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';

import { parse as parseToml } from 'smol-toml';
import type { TomlTable, TomlValue } from 'smol-toml';

export interface ModuleRow {
  name: string;
  entry: string;
  public: readonly string[];
  imports: readonly string[];
}

export interface ProjectConfig {
  root: string;
  modules: ReadonlyMap<string, ModuleRow>;
}

export type ConfigResult = { ok: true; config: ProjectConfig } | { ok: false; reason: string };

type RowResult = { ok: true; row: ModuleRow } | { ok: false; reason: string };

type ModulesResult = { ok: true; modules: Map<string, ModuleRow> } | { ok: false; reason: string };

const configFileName = 'stomme.toml';
const schema = 1n;
const language = 'typescript';
const rowFields = new Set(['entry', 'public', 'imports']);

const errorMessage = (error: unknown) => (error instanceof Error ? error.message : String(error));

const refuse = (reason: string): ConfigResult => ({ ok: false, reason });

const isObject = (value: unknown): value is object => typeof value === 'object' && value !== null;

const isTable = (value: unknown): value is TomlTable =>
  isObject(value) && !Array.isArray(value) && Object.getPrototypeOf(value) === null;

const isStringList = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');

const isStringTable = (value: unknown): value is Record<string, string> =>
  isTable(value) && Object.values(value).every((item) => typeof item === 'string');

const formatValue = (value: unknown) => {
  if (typeof value === 'bigint') {
    return value.toString();
  }

  return JSON.stringify(value);
};

const readPackageVersion = (): string => {
  const text = readFileSync(new URL('../package.json', import.meta.url), 'utf8');
  const manifest: unknown = JSON.parse(text);

  if (!isObject(manifest) || !('version' in manifest)) {
    throw new TypeError('package.json has no version');
  }

  const { version } = manifest;

  if (typeof version !== 'string') {
    throw new TypeError('package.json has no string version');
  }

  return version;
};

let packageVersion: string | undefined;

const getPackageVersion = () => (packageVersion ??= readPackageVersion());

const pathProblem = (name: string, path: string): string | undefined => {
  if (isAbsolute(path)) {
    return `has a module \`${name}\` path \`${path}\` that is absolute; use a path relative to the project root`;
  }

  if (path.split('/').some((part) => part === '..')) {
    return `has a module \`${name}\` path \`${path}\` with a \`..\` part; use a path inside the project`;
  }

  return undefined;
};

const rowFieldProblem = (name: string, value: TomlTable): string | undefined => {
  for (const key of Object.keys(value).toSorted()) {
    if (!rowFields.has(key)) {
      return `has a module \`${name}\` with \`${key}\`; read only \`entry\`, \`public\`, and \`imports\``;
    }
  }

  return undefined;
};

const rowPathProblem = (
  name: string,
  entry: string,
  publicFiles: readonly string[],
): string | undefined => {
  for (const path of [entry, ...publicFiles]) {
    const problem = pathProblem(name, path);

    if (problem !== undefined) {
      return problem;
    }
  }

  return undefined;
};

const rowImportProblem = (
  name: string,
  imports: readonly string[],
  moduleNames: ReadonlySet<string>,
): string | undefined => {
  for (const target of imports) {
    if (!moduleNames.has(target)) {
      return `has a module \`${name}\` that imports \`${target}\`, which has no \`[modules.${target}]\` row`;
    }
  }

  return undefined;
};

const parseRow = (
  name: string,
  value: TomlValue | undefined,
  moduleNames: ReadonlySet<string>,
): RowResult => {
  if (!isTable(value)) {
    return { ok: false, reason: `has a module \`${name}\` that is not a table` };
  }

  const fieldProblem = rowFieldProblem(name, value);

  if (fieldProblem !== undefined) {
    return { ok: false, reason: fieldProblem };
  }

  const entry = value.entry;

  if (typeof entry !== 'string') {
    return { ok: false, reason: `has a module \`${name}\` without a string \`entry\`` };
  }

  const publicFiles = value.public ?? [];

  if (!isStringList(publicFiles)) {
    return {
      ok: false,
      reason: `has a module \`${name}\` with \`public\` that is not a list of strings`,
    };
  }

  const imports = value.imports ?? [];

  if (!isStringList(imports)) {
    return {
      ok: false,
      reason: `has a module \`${name}\` with \`imports\` that is not a list of strings`,
    };
  }

  const pathRefusal = rowPathProblem(name, entry, publicFiles);

  if (pathRefusal !== undefined) {
    return { ok: false, reason: pathRefusal };
  }

  const importProblem = rowImportProblem(name, imports, moduleNames);

  if (importProblem !== undefined) {
    return { ok: false, reason: importProblem };
  }

  return { ok: true, row: { name, entry, public: publicFiles, imports } };
};

const parseModules = (value: TomlValue | undefined): ModulesResult => {
  if (value === undefined) {
    return { ok: true, modules: new Map() };
  }

  if (!isTable(value)) {
    return { ok: false, reason: 'has `modules` that is not a table' };
  }

  const names = Object.keys(value).toSorted();
  const moduleNames = new Set(names);
  const modules = new Map<string, ModuleRow>();

  for (const name of names) {
    const result = parseRow(name, value[name], moduleNames);

    if (!result.ok) {
      return result;
    }

    modules.set(name, result.row);
  }

  return { ok: true, modules };
};

const schemaProblem = (table: TomlTable): string | undefined => {
  const value = table.schema;

  if (value === undefined) {
    return `is missing \`schema\`; use \`schema = ${schema}\``;
  }

  if (value !== schema) {
    return `has \`schema = ${formatValue(value)}\`; this package reads \`schema = ${schema}\``;
  }

  return undefined;
};

const versionProblem = (table: TomlTable): string | undefined => {
  const value = table.version;

  if (value === undefined) {
    return 'is missing `version`';
  }

  if (typeof value !== 'string') {
    return 'has `version` that is not a string';
  }

  const expected = getPackageVersion();

  if (value !== expected) {
    return `has \`version = ${formatValue(value)}\`; this package is version ${expected}`;
  }

  return undefined;
};

const languageProblem = (table: TomlTable): string | undefined => {
  const value = table.language;

  if (value === undefined) {
    return 'is missing `language`';
  }

  if (typeof value !== 'string') {
    return 'has `language` that is not a string';
  }

  if (value !== language) {
    return `has \`language = ${formatValue(value)}\`; this package reads only \`language = "${language}"\``;
  }

  return undefined;
};

const layersProblem = (table: TomlTable): string | undefined => {
  const value = table.layers;

  if (value !== undefined && !isStringList(value)) {
    return 'has `layers` that is not a list of strings';
  }

  return undefined;
};

const rolesProblem = (table: TomlTable): string | undefined => {
  const value = table.roles;

  if (value !== undefined && !isStringTable(value)) {
    return 'has `roles` that is not a table of strings';
  }

  return undefined;
};

export const parseConfig = (text: string, root: string): ConfigResult => {
  let table: TomlTable;

  try {
    table = parseToml(text, { integersAsBigInt: true });
  } catch (error) {
    return refuse(`is not valid TOML: ${errorMessage(error)}`);
  }

  const schemaRefusal = schemaProblem(table);

  if (schemaRefusal !== undefined) {
    return refuse(schemaRefusal);
  }

  const versionRefusal = versionProblem(table);

  if (versionRefusal !== undefined) {
    return refuse(versionRefusal);
  }

  const languageRefusal = languageProblem(table);

  if (languageRefusal !== undefined) {
    return refuse(languageRefusal);
  }

  const layersRefusal = layersProblem(table);

  if (layersRefusal !== undefined) {
    return refuse(layersRefusal);
  }

  const rolesRefusal = rolesProblem(table);

  if (rolesRefusal !== undefined) {
    return refuse(rolesRefusal);
  }

  const modules = parseModules(table.modules);

  if (!modules.ok) {
    return modules;
  }

  return { ok: true, config: { root, modules: modules.modules } };
};

const fileByDirectory = new Map<string, string | null>();
const parseByFile = new Map<string, { mtimeMs: number; result: ConfigResult }>();

const findConfigFile = (directory: string): string | undefined => {
  const visited: string[] = [];
  let current = resolve(directory);
  let found: string | null | undefined;

  while (found === undefined) {
    const cached = fileByDirectory.get(current);

    if (cached !== undefined) {
      found = cached;

      break;
    }

    visited.push(current);
    const candidate = join(current, configFileName);

    if (existsSync(candidate)) {
      found = candidate;

      break;
    }

    const parent = dirname(current);

    if (parent === current) {
      found = null;

      break;
    }

    current = parent;
  }

  for (const folder of visited) {
    fileByDirectory.set(folder, found ?? null);
  }

  return found ?? undefined;
};

export const findConfig = (directory: string): ConfigResult | undefined => {
  const path = findConfigFile(directory);

  if (path === undefined) {
    return undefined;
  }

  let text: string;

  try {
    text = readFileSync(path, 'utf8');
  } catch (error) {
    return refuse(`could not read the file: ${errorMessage(error)}`);
  }

  let mtimeMs: number;

  try {
    mtimeMs = statSync(path).mtimeMs;
  } catch {
    return parseConfig(text, dirname(path));
  }

  const cached = parseByFile.get(path);

  if (cached?.mtimeMs === mtimeMs) {
    return cached.result;
  }

  const result = parseConfig(text, dirname(path));

  parseByFile.set(path, { mtimeMs, result });

  return result;
};
