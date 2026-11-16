import type { Definition, ESTree, Variable } from '@oxlint/plugins';

// A name that starts with `_` marks a parameter or a destructured rest sibling as unused.
export const isUnusedMarker = (definition: Definition, variable: Variable): boolean => {
  if (!definition.name.name.startsWith('_')) {
    return false;
  }

  if (variable.references.some((reference) => reference.isRead())) {
    return false;
  }

  if (definition.type === 'Parameter') {
    return true;
  }

  return definition.node.type === 'VariableDeclarator' && definition.node.id.type !== 'Identifier';
};

// Imports keep the exporter's name, and type-only declarations are checked by their own visitors.
export const isOwnValueBinding = (definition: Definition): boolean =>
  definition.type !== 'ImportBinding' && !definition.node.type.startsWith('TS');

// Each binding declared in the file, once, after the whole program has been visited.
export const forEachBinding = (
  scopes: Iterable<{ variables: Variable[] }>,
  visit: (definition: Definition, variable: Variable) => void,
): void => {
  const visited = new Set<ESTree.Node>();
  const variables = Array.from(scopes).flatMap((scope) => scope.variables);

  for (const variable of variables) {
    for (const definition of variable.defs.filter(isOwnValueBinding)) {
      if (!visited.has(definition.name)) {
        visited.add(definition.name);
        visit(definition, variable);
      }
    }
  }
};
