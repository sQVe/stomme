import type { Rule } from '@oxlint/plugins';

import { isDisableDirective } from '../shared/comments.ts';

const reasonPattern = /\s--\s*\S/;

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
          if (isDisableDirective(comment) && !reasonPattern.test(comment.value)) {
            context.report({ node: comment, messageId: 'missing' });
          }
        }
      },
    };
  },
};
