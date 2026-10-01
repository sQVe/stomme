import type { Comment } from '@oxlint/plugins';

const directivePattern = /^\s*(?:eslint|oxlint)-(?:disable|enable)\b/;
const disablePattern = /^\s*(?:eslint|oxlint)-disable(?:-next-line|-line)?\b/;
const licenseTagPattern = /@license\b|SPDX-License-Identifier:/;
// A license banner opens with the notice; a comment that only mentions copyright is prose.
const copyrightNoticePattern = /^[\s*]*copyright\b/i;

// Text a linter or the TypeScript compiler reads, not prose for people.
export const isToolDirective = (comment: Comment): boolean => {
  const tripleSlash = comment.type === 'Line' && comment.value.startsWith('/ <reference');

  return tripleSlash || directivePattern.test(comment.value) || /^\s*@ts-/.test(comment.value);
};

export const isDisableDirective = (comment: Comment): boolean => disablePattern.test(comment.value);

// `/*!` marks a comment that minifiers keep, which is how license banners are written.
export const isLicenseHeader = (comment: Comment): boolean =>
  comment.value.startsWith('!') ||
  licenseTagPattern.test(comment.value) ||
  copyrightNoticePattern.test(comment.value);
