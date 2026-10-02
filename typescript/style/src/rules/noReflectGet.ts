import type { Rule } from '@oxlint/plugins';

import { isGlobalReflectMethodCall } from '../shared/reflectMethod.ts';

const receiverArgumentCount = 3;

export const noReflectGetRule: Rule = {
  meta: {
    type: 'problem',
    schema: [],
    messages: {
      reflectGet:
        'Replace `Reflect.get` with typed property access. Parse dynamic input into a named type before reading it.',
    },
  },
  create(context) {
    return {
      CallExpression(node) {
        // A receiver keeps `this` for getters, as Proxy traps need; property access cannot pass one.
        const passesReceiver = node.arguments.length >= receiverArgumentCount;

        if (!passesReceiver && isGlobalReflectMethodCall(context.sourceCode, node, 'get')) {
          context.report({ node, messageId: 'reflectGet' });
        }
      },
    };
  },
};
