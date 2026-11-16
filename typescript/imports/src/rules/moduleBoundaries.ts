import { existsSync } from 'node:fs';
import { posix } from 'node:path';

import type { ESTree, Rule } from '@oxlint/plugins';

import type { ModuleRow, ProjectConfig } from '../config.ts';
import { findConfig } from '../config.ts';

interface LocatedModule {
  row: ModuleRow;
  entry: string;
  location: string;
  isFolder: boolean;
}

const stripExtension = (path: string): string => path.replace(/\.(?:js|jsx|mjs|ts|tsx|mts)$/, '');

const packageRoots = new Map<string, string | undefined>();

const packageRootOf = (directory: string): string | undefined => {
  if (packageRoots.has(directory)) {
    return packageRoots.get(directory);
  }

  const parent = posix.dirname(directory);
  let found: string | undefined;

  if (existsSync(posix.join(directory, 'package.json'))) {
    found = directory;
  } else if (parent !== directory) {
    found = packageRootOf(parent);
  }

  packageRoots.set(directory, found);

  return found;
};

// The src folder of the package that holds the file. Without a package.json above the file, the
// last src folder in the path counts.
const sourceRootOf = (path: string): string | undefined => {
  const packageRoot = packageRootOf(posix.dirname(path));

  if (packageRoot !== undefined) {
    return path.startsWith(`${packageRoot}/src/`) ? `${packageRoot}/src` : undefined;
  }

  const segments = path.split('/');
  const index = segments.lastIndexOf('src');

  return index === -1 ? undefined : segments.slice(0, index + 1).join('/');
};

const isSupportFile = (sourceRoot: string, path: string): boolean => {
  if (/\.test\.tsx?$/.test(path)) {
    return true;
  }

  const relative = path.slice(sourceRoot.length + 1);

  return relative.split('/').slice(0, -1).includes('fixtures');
};

const moduleEntryOf = (config: ProjectConfig, row: ModuleRow): string =>
  stripExtension(posix.normalize(posix.join(config.root, row.entry)));

const isFolderModule = (entry: string): boolean =>
  posix.basename(entry) === posix.basename(posix.dirname(entry));

const locateModules = (config: ProjectConfig): LocatedModule[] =>
  [...config.modules.values()].map((row) => {
    const entry = moduleEntryOf(config, row);
    const isFolder = isFolderModule(entry);

    return {
      row,
      entry,
      location: isFolder ? posix.dirname(entry) : entry,
      isFolder,
    };
  });

const containsPath = (module: LocatedModule, path: string): boolean =>
  module.isFolder
    ? path === module.location || path.startsWith(`${module.location}/`)
    : path === module.location;

const moduleOf = (modules: readonly LocatedModule[], path: string): LocatedModule | undefined =>
  modules
    .filter((module) => containsPath(module, path))
    .toSorted((first, second) => second.location.length - first.location.length)[0];

const publicLocationsOf = (config: ProjectConfig, module: LocatedModule): string[] => [
  module.entry,
  ...module.row.public.map((file) =>
    stripExtension(posix.normalize(posix.join(config.root, file))),
  ),
];

const isPublicPath = (config: ProjectConfig, module: LocatedModule, path: string): boolean =>
  publicLocationsOf(config, module).includes(path);

const containingFoldersOf = (modules: readonly LocatedModule[], path: string): string[] =>
  modules
    .filter((module) => module.isFolder && containsPath(module, path))
    .map((module) => module.location);

const allowedTargetsOf = (
  config: ProjectConfig,
  ownModule: LocatedModule | undefined,
): Set<string> => {
  const importedNames = new Set<string>();

  for (const row of config.modules.values()) {
    for (const target of row.imports) {
      importedNames.add(target);
    }
  }

  const allowedTargets = new Set<string>();

  if (ownModule !== undefined) {
    allowedTargets.add(ownModule.row.name);

    for (const target of ownModule.row.imports) {
      allowedTargets.add(target);
    }
  }

  for (const row of config.modules.values()) {
    if (!importedNames.has(row.name)) {
      continue;
    }

    for (const target of row.imports) {
      allowedTargets.add(target);
    }
  }

  return allowedTargets;
};

const literalSourceOf = (source: ESTree.Expression): string | undefined => {
  if (source.type === 'Literal' && typeof source.value === 'string') {
    return source.value;
  }

  if (source.type !== 'TemplateLiteral' || source.expressions.length > 0) {
    return undefined;
  }

  return source.quasis[0]?.value.cooked ?? undefined;
};

export const moduleBoundariesRule: Rule = {
  meta: {
    type: 'problem',
    schema: [],
    messages: {
      notAllowed:
        'Module "{{source}}" must not import module "{{target}}". Add "{{target}}" to imports of [modules.{{source}}] in stomme.toml if the edge is intended.',
      private:
        'Import module "{{folder}}" through its entry file, not its private file "{{file}}". Add the file to public of [modules.{{folder}}] in stomme.toml if other modules depend on it.',
      unknownFile:
        'File "{{file}}" is not in a module. Add a [modules.<name>] row with its entry in stomme.toml.',
      computedImport:
        'Import a literal path so the module table can follow it. A computed import() hides its target.',
      noConfig:
        'No stomme.toml above this file. Add one with a [modules.<name>] row for each module.',
      brokenConfig: 'stomme.toml {{reason}}',
    },
  },
  create(context) {
    const filename = context.filename.replaceAll('\\', '/');
    const sourceRoot = sourceRootOf(filename);

    if (sourceRoot === undefined) {
      return {};
    }

    const result = findConfig(posix.dirname(filename));

    if (result === undefined) {
      return {
        Program(node) {
          context.report({ node, messageId: 'noConfig' });
        },
      };
    }

    if (!result.ok) {
      return {
        Program(node) {
          context.report({ node, messageId: 'brokenConfig', data: { reason: result.reason } });
        },
      };
    }

    const { config } = result;

    if (config.modules.size === 0) {
      return {};
    }

    const modules = locateModules(config);
    const ownPath = stripExtension(filename);
    const ownModule = moduleOf(modules, ownPath);
    const support = isSupportFile(sourceRoot, filename);
    const sourceFolders = containingFoldersOf(modules, ownPath);
    const sourceName = ownModule?.row.name ?? ownPath.slice(sourceRoot.length + 1);
    const allowedTargets = allowedTargetsOf(config, ownModule);

    const checkPrivateFile = (node: ESTree.Node, target: LocatedModule, targetPath: string) => {
      if (!target.isFolder || targetPath === target.location) {
        return;
      }

      if (isPublicPath(config, target, targetPath)) {
        return;
      }

      const isContained = sourceFolders.some(
        (folder) => folder === target.location || folder.startsWith(`${target.location}/`),
      );

      if (!isContained) {
        context.report({
          node,
          messageId: 'private',
          data: {
            folder: target.row.name,
            file: targetPath.slice(target.location.length + 1),
          },
        });
      }
    };

    const checkTarget = (node: ESTree.Node, specifier: string) => {
      const targetPath = stripExtension(
        posix.normalize(posix.join(posix.dirname(filename), specifier)),
      );

      if (!targetPath.startsWith(`${sourceRoot}/`)) {
        return;
      }

      const target = moduleOf(modules, targetPath);

      if (!support && target !== undefined) {
        checkPrivateFile(node, target, targetPath);
      }

      if (ownModule === undefined || target === undefined || target === ownModule) {
        return;
      }

      if (support) {
        if (!allowedTargets.has(target.row.name)) {
          context.report({
            node,
            messageId: 'notAllowed',
            data: { source: sourceName, target: target.row.name },
          });
        }

        return;
      }

      if (!ownModule.row.imports.includes(target.row.name)) {
        context.report({
          node,
          messageId: 'notAllowed',
          data: { source: ownModule.row.name, target: target.row.name },
        });
      }
    };

    const check = (node: ESTree.Node, source: ESTree.Expression) => {
      const specifier = literalSourceOf(source);

      if (specifier === undefined || !specifier.startsWith('.')) {
        return;
      }

      checkTarget(node, specifier);
    };

    const checkSource = (node: ESTree.Node, source: ESTree.Expression | null | undefined) => {
      if (source !== null && source !== undefined) {
        check(node, source);
      }
    };

    return {
      Program(node) {
        if (ownModule === undefined && !support) {
          context.report({
            node,
            messageId: 'unknownFile',
            data: { file: filename.slice(sourceRoot.length + 1) },
          });
        }
      },
      ImportDeclaration(node) {
        check(node, node.source);
      },
      ExportAllDeclaration(node) {
        check(node, node.source);
      },
      ExportNamedDeclaration(node) {
        checkSource(node, node.source);
      },
      ImportExpression(node) {
        const specifier = literalSourceOf(node.source);

        if (specifier === undefined) {
          if (!support) {
            context.report({ node, messageId: 'computedImport' });
          }

          return;
        }

        if (specifier.startsWith('.')) {
          checkTarget(node, specifier);
        }
      },
      TSImportType(node) {
        check(node, node.source);
      },
    };
  },
};
