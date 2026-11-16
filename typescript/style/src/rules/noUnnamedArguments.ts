import type { ESTree, Rule } from '@oxlint/plugins';

import { isWrappedExpression, unwrapExpressionAndChain } from '../shared/wrappedExpression.ts';

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

const isTransparentParent = (node: ESTree.Node): boolean =>
  isWrappedExpression(node) || node.type === 'ChainExpression';

const isMemberObject = (parent: ESTree.Node, child: ESTree.Node): boolean =>
  parent.type === 'MemberExpression' && parent.object === child;

const isCallee = (parent: ESTree.Node, child: ESTree.Node): boolean =>
  parent.type === 'CallExpression' && parent.callee === child;

const continuesChain = (parent: ESTree.Node, child: ESTree.Node): boolean =>
  isTransparentParent(parent) || isMemberObject(parent, child) || isCallee(parent, child);

const chainTop = (node: ESTree.Node): ESTree.Node => {
  const parent = node.parent;

  if (parent === null) {
    return node;
  }

  return continuesChain(parent, node) ? chainTop(parent) : node;
};

const callCount = (node: ESTree.Node): number => {
  const unwrapped = unwrapExpressionAndChain(node);

  if (unwrapped.type === 'CallExpression') {
    return 1 + callCount(unwrapped.callee);
  }

  return unwrapped.type === 'MemberExpression' ? callCount(unwrapped.object) : 0;
};

const isSideExpression = (
  node: ESTree.Node,
): node is ESTree.LogicalExpression | ESTree.BinaryExpression =>
  node.type === 'LogicalExpression' || node.type === 'BinaryExpression';

const isFromArray = (parent: ESTree.ArrayExpression, child: ESTree.Node): boolean =>
  parent.elements.some((element) => element === child);

const isFromSide = (
  parent: ESTree.LogicalExpression | ESTree.BinaryExpression,
  child: ESTree.Node,
): boolean => parent.left === child || parent.right === child;

const isFromTemplate = (parent: ESTree.TemplateLiteral, child: ESTree.Node): boolean =>
  parent.expressions.some((expression) => expression === child);

const isFromPropertyValue = (parent: ESTree.Node, child: ESTree.Node): boolean =>
  parent.type === 'Property' && 'value' in parent && parent.value === child;

const isFromComputedPropertyKey = (parent: ESTree.Node, child: ESTree.Node): boolean => {
  if (parent.type !== 'Property') {
    return false;
  }

  if (!('computed' in parent) || !parent.computed) {
    return false;
  }

  return 'key' in parent && parent.key === child;
};

const isFromPropertySlot = (parent: ESTree.Node, child: ESTree.Node): boolean =>
  isFromPropertyValue(parent, child) || isFromComputedPropertyKey(parent, child);

const isFromObject = (parent: ESTree.ObjectExpression, child: ESTree.Node): boolean =>
  parent.properties.some((property) => property === child);

const isEvaluatedChild = (parent: ESTree.Node, child: ESTree.Node): boolean => {
  if (isTransparentParent(parent) || parent.type === 'SpreadElement') {
    return true;
  }

  if (parent.type === 'ArrayExpression') {
    return isFromArray(parent, child);
  }

  if (isSideExpression(parent)) {
    return isFromSide(parent, child);
  }

  if (parent.type === 'UnaryExpression') {
    return parent.argument === child;
  }

  if (parent.type === 'TemplateLiteral') {
    return isFromTemplate(parent, child);
  }

  if (parent.type === 'Property') {
    return isFromPropertySlot(parent, child);
  }

  if (parent.type === 'ObjectExpression') {
    return isFromObject(parent, child);
  }

  if (parent.type === 'MemberExpression') {
    return parent.computed && parent.property === child;
  }

  return false;
};

const reachesArgument = (node: ESTree.Node): boolean => {
  const parent = node.parent;

  if (parent === null) {
    return false;
  }

  if (isEvaluatedChild(parent, node)) {
    return reachesArgument(parent);
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
        // Only the first call of a chain checks it, so each chain is reported once.
        if (callCount(node.callee) > 0) {
          return;
        }

        const top = chainTop(node);

        if (callCount(top) < 2) {
          return;
        }

        if (reachesArgument(top)) {
          context.report({ node: top, messageId: 'callChain' });
        }
      },
    };
  },
};
