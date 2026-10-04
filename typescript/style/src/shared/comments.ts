import type { Comment } from '@oxlint/plugins';

const directivePattern = /^\s*(?:eslint|oxlint)-(?:disable|enable)\b/;
const disablePattern = /^\s*(?:eslint|oxlint)-disable(?:-next-line|-line)?\b/;
// The rule names sit between the directive and the ` -- ` that starts the reason.
const lineDisablePattern = /^\s*(?:eslint|oxlint)-disable-(next-line|line)\b(.*?)(?:\s--|$)/s;
const reasonPattern = /\s--\s*\S/;
const licenseTagPattern = /@license\b|SPDX-License-Identifier:/;
// A license banner opens with the notice; a comment that only mentions copyright is prose.
const copyrightNoticePattern = /^[\s*]*copyright\b/i;

// Text a linter or the TypeScript compiler reads, not prose for people.
export const isToolDirective = (comment: Comment): boolean => {
  const tripleSlash = comment.type === 'Line' && comment.value.startsWith('/ <reference');

  return tripleSlash || directivePattern.test(comment.value) || /^\s*@ts-/.test(comment.value);
};

export const isDisableDirective = (comment: Comment): boolean => disablePattern.test(comment.value);

export const hasDisableReason = (comment: Comment): boolean => reasonPattern.test(comment.value);

// A directive that lists no rules turns off every rule, but it does not name `ruleName`.
export const lineDisableScope = (
  comment: Comment,
  ruleName: string,
): 'line' | 'next-line' | undefined => {
  const match = lineDisablePattern.exec(comment.value);

  if (match === null) {
    return undefined;
  }

  const [, scope, ruleList = ''] = match;
  const ruleNames = ruleList.split(',').map((name) => name.trim());

  if (!ruleNames.includes(ruleName)) {
    return undefined;
  }

  return scope === 'line' ? 'line' : 'next-line';
};

// `/*!` marks a comment that minifiers keep, which is how license banners are written.
export const isLicenseHeader = (comment: Comment): boolean =>
  comment.value.startsWith('!') ||
  licenseTagPattern.test(comment.value) ||
  copyrightNoticePattern.test(comment.value);
