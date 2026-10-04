import type { ESTree, Rule, SourceCode } from '@oxlint/plugins';

import { hasDisableReason, lineDisableScope } from '../shared/comments.ts';

type TypeAssertion = ESTree.TSAsExpression | ESTree.TSTypeAssertion;

// `SAFETY:` followed by text, not preceded by a letter, digit, or underscore.
const safetyPattern = /(?:^|[^\p{L}\p{N}_])SAFETY\s*:\s*\S/u;

// Comments above these statements explain the assertions inside them.
const commentOwnerKinds = new Set([
  'ExpressionStatement',
  'PropertyDefinition',
  'ReturnStatement',
  'ThrowStatement',
  'VariableDeclaration',
]);

const isConstAssertion = (node: TypeAssertion): boolean => {
  const annotation = node.typeAnnotation;

  return (
    annotation.type === 'TSTypeReference' &&
    annotation.typeName.type === 'Identifier' &&
    annotation.typeName.name === 'const'
  );
};

const hasSafetyCommentBefore = (
  sourceCode: SourceCode,
  owner: ESTree.Node,
  assertion: TypeAssertion,
): boolean =>
  sourceCode
    .getCommentsBefore(owner)
    .some((comment) => comment.end <= assertion.start && safetyPattern.test(comment.value));

// The loop whose head declares `node`; the comment above that loop covers the declaration.
const loopHeadOf = (node: ESTree.Node): ESTree.Node | undefined => {
  const { parent } = node;

  if (parent?.type === 'ForStatement') {
    return parent.init === node ? parent : undefined;
  }

  const isForInOrOf = parent?.type === 'ForInStatement' || parent?.type === 'ForOfStatement';

  return isForInOrOf && parent.left === node ? parent : undefined;
};

const exportOf = (owner: ESTree.Node): ESTree.Node | undefined => {
  const parent = owner.parent;
  const exportsOwner = parent?.type === 'ExportNamedDeclaration' && parent.declaration === owner;

  return exportsOwner ? parent : undefined;
};

// Search from the assertion outward, stopping at the statement that owns it.
const hasSafetyComment = (sourceCode: SourceCode, assertion: TypeAssertion): boolean => {
  let current: ESTree.Node = assertion;

  while (!hasSafetyCommentBefore(sourceCode, current, assertion)) {
    if (commentOwnerKinds.has(current.type)) {
      const loop = loopHeadOf(current);

      if (loop !== undefined) {
        return hasSafetyCommentBefore(sourceCode, loop, assertion);
      }

      const exported = exportOf(current);

      return exported !== undefined && hasSafetyCommentBefore(sourceCode, exported, assertion);
    }

    if (current.parent.type === 'Program') {
      return false;
    }

    current = current.parent;
  }

  return true;
};

// Lines where a one-line directive with a reason turns off the rule this comment replaces.
const disabledLinesOf = (sourceCode: SourceCode): Set<number> => {
  const disabled = new Set<number>();

  for (const comment of sourceCode.getAllComments()) {
    const scope = lineDisableScope(comment, 'typescript/no-unsafe-type-assertion');

    if (scope === undefined || !hasDisableReason(comment)) {
      continue;
    }

    disabled.add(scope === 'line' ? comment.loc.start.line : comment.loc.end.line + 1);
  }

  return disabled;
};

export const requireSafetyCommentForTypeAssertionRule: Rule = {
  meta: {
    type: 'problem',
    schema: [],
    messages: {
      missingSafetyComment:
        'This type assertion has no `SAFETY:` comment. State the checked invariant right before the assertion or its statement, or disable `typescript/no-unsafe-type-assertion` with a reason after " -- " in an `oxlint-disable-next-line` comment on the line above the assertion or an `oxlint-disable-line` comment on its line.',
    },
  },
  create(context) {
    let disabledLines: Set<number> | undefined;

    const isJustified = (node: TypeAssertion): boolean => {
      if (hasSafetyComment(context.sourceCode, node)) {
        return true;
      }

      disabledLines ??= disabledLinesOf(context.sourceCode);

      return disabledLines.has(node.loc.start.line);
    };

    const checkAssertion = (node: TypeAssertion) => {
      if (!isConstAssertion(node) && !isJustified(node)) {
        context.report({ node, messageId: 'missingSafetyComment' });
      }
    };

    return {
      TSAsExpression: checkAssertion,
      TSTypeAssertion: checkAssertion,
    };
  },
};
