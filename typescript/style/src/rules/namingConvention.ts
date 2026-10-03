import type { Definition, ESTree, Reference, Rule, SourceCode, Variable } from '@oxlint/plugins';

import { forEachBinding, isUnusedMarker } from '../shared/bindings.ts';
import { resolveVariable } from '../shared/variables.ts';
import { unwrapExpression } from '../shared/wrappedExpression.ts';

type Identifier = Extract<ESTree.Node, { type: 'Identifier' }>;

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

// React returns a component from these wrappers, whatever they wrap.
const componentWrappers = new Set(['memo', 'forwardRef']);

const exportedNameOf = (name: ESTree.ModuleExportName): string =>
  name.type === 'Literal' ? name.value : name.name;

const importSpecifierTypes = new Set<string>([
  'ImportSpecifier',
  'ImportDefaultSpecifier',
  'ImportNamespaceSpecifier',
]);

const isImportSpecifier = (node: ESTree.Node): node is ESTree.ImportDeclarationSpecifier =>
  importSpecifierTypes.has(node.type);

// `import { name }` keeps the exporter's name; a rename, a default, a namespace import, or an
// `import name =` is the file's choice.
const isChosenImportName = (definition: Definition): boolean => {
  const specifier = definition.node;

  if (specifier.type === 'TSImportEqualsDeclaration') {
    return true;
  }

  if (!isImportSpecifier(specifier)) {
    return false;
  }

  return (
    specifier.type !== 'ImportSpecifier' ||
    exportedNameOf(specifier.imported) !== specifier.local.name
  );
};

// The import specifier that declares this name, when the module it imports from is `react`.
const reactImportOf = (
  sourceCode: SourceCode,
  identifier: Identifier,
): ESTree.ImportDeclarationSpecifier | undefined => {
  const definition = resolveVariable(sourceCode, identifier)?.defs[0];

  if (definition?.type !== 'ImportBinding') {
    return undefined;
  }

  const specifier = definition.node;

  if (!isImportSpecifier(specifier)) {
    return undefined;
  }

  const declaration = specifier.parent;

  const fromReact =
    declaration.type === 'ImportDeclaration' && declaration.source.value === 'react';

  return fromReact ? specifier : undefined;
};

// `memo` and `memo as alias` from a named import, or `React.memo` on the default or namespace
// import. A local or another module's function of the same name is not React's.
const wrapperNameOf = (sourceCode: SourceCode, callee: ESTree.Expression): string | undefined => {
  if (callee.type === 'Identifier') {
    const specifier = reactImportOf(sourceCode, callee);

    return specifier?.type === 'ImportSpecifier' ? exportedNameOf(specifier.imported) : undefined;
  }

  if (
    callee.type !== 'MemberExpression' ||
    callee.computed ||
    callee.object.type !== 'Identifier'
  ) {
    return undefined;
  }

  const specifier = reactImportOf(sourceCode, callee.object);
  const wholeModule = specifier !== undefined && specifier.type !== 'ImportSpecifier';

  return wholeModule ? callee.property.name : undefined;
};

const isComponentWrapper = (sourceCode: SourceCode, node: ESTree.Node): boolean => {
  if (node.type !== 'CallExpression') {
    return false;
  }

  const wrapperName = wrapperNameOf(sourceCode, node.callee);

  return wrapperName !== undefined && componentWrappers.has(wrapperName);
};

// `<Icon />` reads the binding as a component; `<icons.Check />` and `{Icon}` do not.
const isPlainTag = (node: ESTree.Node): boolean =>
  node.type === 'JSXIdentifier' && node.parent.type === 'JSXOpeningElement';

// The types declare a plain identifier, but Oxlint records JSX tags as references too.
const isTagReference = (reference: Reference): boolean => isPlainTag(reference.identifier);

// JSX needs an uppercase name to render a binding as a component, whatever kind of binding it is.
const isRenderedAsTag = (variable: Variable): boolean => variable.references.some(isTagReference);

const isCamelCase = (name: string): boolean => /^[a-z][a-zA-Z0-9]*$/.test(name);

const isPascalCase = (name: string): boolean => /^[A-Z][a-zA-Z0-9]*$/.test(name);

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
      const valid = format === 'camelCase' ? isCamelCase(name) : isPascalCase(name);

      if (!valid) {
        context.report({ node, messageId: 'name', data: { format, name: node.name } });
      }
    };

    const isComponent = (definition: Definition, variable: Variable): boolean => {
      if (isRenderedAsTag(variable)) {
        return true;
      }

      const initializer = constantInitializerOf(definition);

      if (initializer !== undefined && isComponentWrapper(context.sourceCode, initializer)) {
        return true;
      }

      const definedFunction = definedFunctionOf(definition);

      return definedFunction !== undefined && components.has(definedFunction);
    };

    const checkDefinition = (definition: Definition, variable: Variable) => {
      const unusedMarker = isUnusedMarker(definition, variable);
      const name = unusedMarker ? definition.name.name.slice(1) : definition.name.name;

      if (unusedMarker && !name) {
        return;
      }

      const componentName = /^[A-Z]/.test(name) && isComponent(definition, variable);

      const pascalCase = definition.type === 'ClassName' || componentName;

      checkName(definition.name, pascalCase ? 'PascalCase' : 'camelCase', name);
    };

    // The file cannot see whether an imported value is a class or a component, so a chosen import
    // name may use either case unless JSX renders it.
    const checkImport = (definition: Definition, variable: Variable) => {
      if (isRenderedAsTag(variable)) {
        checkName(definition.name, 'PascalCase');
      } else if (!isPascalCase(definition.name.name)) {
        checkName(definition.name, 'camelCase');
      }
    };

    const checkImports = () => {
      for (const scope of context.sourceCode.scopeManager.scopes) {
        for (const variable of scope.variables) {
          for (const definition of variable.defs.filter(isChosenImportName)) {
            checkImport(definition, variable);
          }
        }
      }
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
        checkImports();
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
