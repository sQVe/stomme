import type { Definition, ESTree, Rule, Variable } from '@oxlint/plugins';

import { forEachBinding, isUnusedMarker } from '../shared/bindings.ts';
import { unwrapExpression } from '../shared/wrappedExpression.ts';

const isFunction = (node: ESTree.Node): boolean =>
  node.type === 'FunctionDeclaration' ||
  node.type === 'FunctionExpression' ||
  node.type === 'ArrowFunctionExpression';

const isJsx = (node: ESTree.Node): boolean => {
  const value = unwrapExpression(node);

  if (value.type === 'ConditionalExpression') {
    return isJsx(value.consequent) || isJsx(value.alternate);
  }

  if (value.type === 'LogicalExpression') {
    return isJsx(value.left) || isJsx(value.right);
  }

  return value.type === 'JSXElement' || value.type === 'JSXFragment';
};

const enclosingFunctionOf = (node: ESTree.Node): ESTree.Node | undefined => {
  let current = node.parent;

  while (current !== null && !isFunction(current)) {
    current = current.parent;
  }

  return current ?? undefined;
};

// A `let` or `var` binding can be reassigned, and a destructured name holds a field of the
// function rather than the function.
const constantInitializerOf = (definition: Definition): ESTree.Node | undefined => {
  const declarator = definition.node;

  if (declarator.type !== 'VariableDeclarator' || declarator.id.type !== 'Identifier') {
    return undefined;
  }

  const declaration = declarator.parent;
  const constant = declaration.type === 'VariableDeclaration' && declaration.kind === 'const';

  return constant && declarator.init !== null ? unwrapExpression(declarator.init) : undefined;
};

const definedFunctionOf = (definition: Definition): ESTree.Node | undefined => {
  // A parameter's definition node is also the function declaration, so check the binding kind.
  if (definition.type === 'FunctionName' && definition.node.type === 'FunctionDeclaration') {
    return definition.node;
  }

  const initializer = constantInitializerOf(definition);

  return initializer !== undefined && isFunction(initializer) ? initializer : undefined;
};

export const namingConventionRule: Rule = {
  meta: {
    type: 'suggestion',
    schema: [],
    messages: { name: 'Use {{format}} for "{{name}}".' },
  },
  create(context) {
    // JSX treats lowercase tags as built-in elements, so components need PascalCase names.
    const components = new Set<ESTree.Node>();

    const checkName = (
      node: ESTree.BindingIdentifier,
      format: 'camelCase' | 'PascalCase',
      name = node.name,
    ) => {
      const pattern = format === 'camelCase' ? /^[a-z][a-zA-Z0-9]*$/ : /^[A-Z][a-zA-Z0-9]*$/;

      if (!pattern.test(name)) {
        context.report({ node, messageId: 'name', data: { format, name: node.name } });
      }
    };

    const checkDefinition = (definition: Definition, variable: Variable) => {
      const unusedMarker = isUnusedMarker(definition, variable);
      const name = unusedMarker ? definition.name.name.slice(1) : definition.name.name;

      if (unusedMarker && !name) {
        return;
      }

      const definedFunction = definedFunctionOf(definition);

      const componentName =
        definedFunction !== undefined && components.has(definedFunction) && /^[A-Z]/.test(name);

      const pascalCase = definition.type === 'ClassName' || componentName;

      checkName(definition.name, pascalCase ? 'PascalCase' : 'camelCase', name);
    };

    return {
      ReturnStatement(node) {
        const returningFunction = enclosingFunctionOf(node);

        if (node.argument !== null && returningFunction !== undefined && isJsx(node.argument)) {
          components.add(returningFunction);
        }
      },
      ArrowFunctionExpression(node) {
        if (node.expression && isJsx(node.body)) {
          components.add(node);
        }
      },
      'Program:exit'() {
        forEachBinding(context.sourceCode.scopeManager.scopes, checkDefinition);
      },
      TSInterfaceDeclaration(node) {
        checkName(node.id, 'PascalCase');
      },
      TSTypeAliasDeclaration(node) {
        checkName(node.id, 'PascalCase');
      },
      TSEnumDeclaration(node) {
        checkName(node.id, 'PascalCase');
      },
      TSTypeParameter(node) {
        checkName(node.name, 'PascalCase');
      },
    };
  },
};
