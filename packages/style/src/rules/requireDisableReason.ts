import type { Rule } from '@oxlint/plugins';

import { hasDisableReason, isDisableDirective } from '../shared/comments.ts';

export const requireDisableReasonRule: Rule = {
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      missing: 'Add a reason after " -- " that says why this rule does not apply here.',
    },
  },
  create(context) {
    return {
      Program() {
        for (const comment of context.sourceCode.getAllComments()) {
          if (isDisableDirective(comment) && !hasDisableReason(comment)) {
            context.report({ node: comment, messageId: 'missing' });
          }
        }
      },
    };
  },
};
