import type { ESTree, Rule } from '@oxlint/plugins';

import { isWrappedExpression } from '../shared/wrappedExpression.ts';

const unwrap = (node: ESTree.Node): ESTree.Node => {
  let current = node;

  while (isWrappedExpression(current) || current.type === 'ChainExpression') {
    current = current.expression;
  }

  return current;
};

const isCallChain = (node: ESTree.CallExpression): boolean => {
  const callee = unwrap(node.callee);

  if (callee.type !== 'MemberExpression') {
    return false;
  }

  return unwrap(callee.object).type === 'CallExpression';
};

const isArgument = (parent: ESTree.Node, child: ESTree.Node): boolean => {
  if (parent.type !== 'CallExpression' && parent.type !== 'NewExpression') {
    return false;
  }

  return parent.arguments.some((argument) => argument === child);
};

const isOptionalPropertySpread = (node: ESTree.ConditionalExpression): boolean => {
  let parent = node.parent;

  while (isWrappedExpression(parent)) {
    parent = parent.parent;
  }

  return parent.type === 'SpreadElement' && parent.parent.type === 'ObjectExpression';
};

const reachesArgument = (node: ESTree.Node): boolean => {
  const parent = node.parent;

  if (parent === null) {
    return false;
  }

  if (isWrappedExpression(parent) || parent.type === 'ChainExpression') {
    return reachesArgument(parent);
  }

  if (parent.type === 'SpreadElement') {
    return reachesArgument(parent);
  }

  if (parent.type === 'ArrayExpression') {
    const fromElement = parent.elements.some((element) => element === node);

    return fromElement && reachesArgument(parent);
  }

  if (parent.type === 'Property') {
    return parent.value === node && reachesArgument(parent);
  }

  if (parent.type === 'ObjectExpression') {
    const fromProperty = parent.properties.some((property) => property === node);

    return fromProperty && reachesArgument(parent);
  }

  return isArgument(parent, node);
};

export const noUnnamedArgumentsRule: Rule = {
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      ternary: 'Assign this ternary to a named variable before passing it as an argument.',
      callChain: 'Assign this call chain to a named variable before passing it as an argument.',
    },
  },
  create(context) {
    return {
      ConditionalExpression(node) {
        if (isOptionalPropertySpread(node)) {
          return;
        }

        if (reachesArgument(node)) {
          context.report({ node, messageId: 'ternary' });
        }
      },
      CallExpression(node) {
        if (!isCallChain(node)) {
          return;
        }

        if (reachesArgument(node)) {
          context.report({ node, messageId: 'callChain' });
        }
      },
    };
  },
};
