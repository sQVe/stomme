import type { ESTree } from '@oxlint/plugins';

type WrappedExpression =
  | ESTree.TSAsExpression
  | ESTree.TSSatisfiesExpression
  | ESTree.TSTypeAssertion
  | ESTree.TSNonNullExpression
  | ESTree.ParenthesizedExpression;

const wrappedExpressionTypes = new Set<string>([
  'TSAsExpression',
  'TSSatisfiesExpression',
  'TSTypeAssertion',
  'TSNonNullExpression',
  'ParenthesizedExpression',
]);

export const isWrappedExpression = (node: ESTree.Node): node is WrappedExpression =>
  wrappedExpressionTypes.has(node.type);

export const unwrapExpression = (node: ESTree.Node): ESTree.Node =>
  isWrappedExpression(node) ? unwrapExpression(node.expression) : node;

export const unwrapExpressionAndChain = (node: ESTree.Node): ESTree.Node => {
  const unwrapped = unwrapExpression(node);

  return unwrapped.type === 'ChainExpression'
    ? unwrapExpressionAndChain(unwrapped.expression)
    : unwrapped;
};
