import type { Rule } from '@oxlint/plugins';

import { isLicenseHeader, isToolDirective } from '../shared/comments.ts';

// Standards, advisories, and model names look like ticket keys but name a stable source.
const standardPrefixes = new Set([
  'AES',
  'ANSI',
  'ASCII',
  'AVX',
  'BASE',
  'CRC',
  'CVE',
  'CWE',
  'DDR',
  'ECDSA',
  'ECMA',
  'ED',
  'EIP',
  'ES',
  'GHSA',
  'GPT',
  'HDMI',
  'HTTP',
  'IEC',
  'IEEE',
  'ISO',
  'LZ',
  'MD',
  'PCI',
  'PEP',
  'PKCS',
  'RFC',
  'RS',
  'SHA',
  'TLS',
  'UCS',
  'USB',
  'UTF',
  'WPA',
]);

const citationPatterns = [
  /\bADR[- ]?\d+/,
  /\b(?:PR|MR|pull request|merge request)\s*#?\d+\b/i,
  /\b(?:issue|ticket|bug|see|fixes|closes|resolves)\s*#\d+\b/i,
  /\(#\d+\)/,
  /\bper (?:the )?(?:code )?review\b/i,
  /\breview(?:er)? (?:feedback|request)\b/i,
  /\brequested (?:in|by|during) (?:the )?(?:code )?review(?:er)?\b/i,
];

const ticketKeyPattern = /\b([A-Z][A-Z0-9]{1,9})-\d+\b/g;

const ticketKeyOf = (text: string): string | undefined => {
  for (const match of text.matchAll(ticketKeyPattern)) {
    const [key, prefix] = match;

    if (prefix !== undefined && !standardPrefixes.has(prefix)) {
      return key;
    }
  }

  return undefined;
};

const citationOf = (text: string): string | undefined => {
  for (const pattern of citationPatterns) {
    const match = pattern.exec(text);

    if (match !== null) {
      return match[0];
    }
  }

  return ticketKeyOf(text);
};

export const noReferenceCommentsRule: Rule = {
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      reference:
        'Comment cites "{{citation}}". State the rule or constraint itself; links to decisions, tickets, and reviews go stale.',
    },
  },
  create(context) {
    return {
      Program() {
        for (const comment of context.sourceCode.getAllComments()) {
          if (isToolDirective(comment) || isLicenseHeader(comment)) {
            continue;
          }

          const citation = citationOf(comment.value);

          if (citation !== undefined) {
            context.report({ node: comment, messageId: 'reference', data: { citation } });
          }
        }
      },
    };
  },
};
