import type { ESTree, Rule } from '@oxlint/plugins';

import { forEachBinding } from '../shared/bindings.ts';

// Standard names such as `id`, `url`, `env`, `args`, `props`, `ref`, and loop counters stay
// allowed: renaming them makes code harder to match against the APIs that use them.
const replacements = new Map([
  ['arr', 'array'],
  ['btn', 'button'],
  ['buf', 'buffer'],
  ['cb', 'callback'],
  ['cfg', 'config'],
  ['cmd', 'command'],
  ['cnt', 'count'],
  ['ctx', 'context'],
  ['desc', 'description'],
  ['el', 'element'],
  ['elem', 'element'],
  ['err', 'error'],
  ['evt', 'event'],
  ['fn', 'callback'],
  ['idx', 'index'],
  ['len', 'length'],
  ['mgr', 'manager'],
  ['msg', 'message'],
  ['num', 'number'],
  ['obj', 'object'],
  ['opts', 'options'],
  ['req', 'request'],
  ['res', 'response'],
  ['resp', 'response'],
  ['str', 'text'],
  ['svc', 'service'],
  ['tmp', 'temporary'],
  ['val', 'value'],
]);

const wordPattern = /[A-Z]?[a-z]+|[A-Z]+(?![a-z])|\d+/g;

// `onBtnClick` gives on, btn, click; `RETRY_CNT` gives retry, cnt; `HTMLElement` gives html, element.
const wordsOf = (name: string): string[] =>
  Array.from(name.matchAll(wordPattern), ([word]) => word.toLowerCase());

export const noAbbreviationsRule: Rule = {
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      abbreviation:
        '"{{name}}" abbreviates "{{word}}". Write the full word, such as "{{replacement}}".',
    },
  },
  create(context) {
    const checkName = (node: ESTree.BindingIdentifier) => {
      for (const word of new Set(wordsOf(node.name))) {
        const replacement = replacements.get(word);

        if (replacement !== undefined) {
          context.report({
            node,
            messageId: 'abbreviation',
            data: { name: node.name, word, replacement },
          });
        }
      }
    };

    return {
      'Program:exit'() {
        forEachBinding(context.sourceCode.scopeManager.scopes, (definition) => {
          checkName(definition.name);
        });
      },
      TSInterfaceDeclaration(node) {
        checkName(node.id);
      },
      TSTypeAliasDeclaration(node) {
        checkName(node.id);
      },
      TSEnumDeclaration(node) {
        checkName(node.id);
      },
      TSTypeParameter(node) {
        checkName(node.name);
      },
    };
  },
};
