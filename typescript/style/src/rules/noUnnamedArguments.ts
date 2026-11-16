import type { ESTree, Rule } from '@oxlint/plugins';

import { isWrappedExpression, unwrapExpressionAndChain } from '../shared/wrappedExpression.ts';

const isCallChain = (node: ESTree.CallExpression): boolean => {
  let callee = unwrapExpressionAndChain(node.callee);

  while (callee.type === 'MemberExpression') {
    callee = unwrapExpressionAndChain(callee.object);
  }

  return callee.type === 'CallExpression';
};

const isArgument = (parent: ESTree.Node, child: ESTree.Node): boolean => {
  if (parent.type !== 'CallExpression' && parent.type !== 'NewExpression') {
    return false;
  }

  return parent.arguments.some((argument) => argument === child);
};

const isEmptyObjectExpression = (node: ESTree.Node): boolean => {
  const unwrapped = unwrapExpressionAndChain(node);

  return unwrapped.type === 'ObjectExpression' && unwrapped.properties.length === 0;
};

const isOptionalPropertySpread = (node: ESTree.ConditionalExpression): boolean => {
  let parent = node.parent;

  while (isWrappedExpression(parent)) {
    parent = parent.parent;
  }

  if (parent.type !== 'SpreadElement' || parent.parent.type !== 'ObjectExpression') {
    return false;
  }

  return isEmptyObjectExpression(node.consequent) || isEmptyObjectExpression(node.alternate);
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
