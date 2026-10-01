import { ruleTester } from '../fixtures/ruleTester.ts';
import { noReferenceCommentsRule } from './noReferenceComments.ts';

const reference = (citation: string) => ({ messageId: 'reference', data: { citation } });

ruleTester.run('no-reference-comments', noReferenceCommentsRule, {
  valid: [
    '// Retry once: the first request can race the cache warmup.\nexport const retries = 1;',
    '// Encode as UTF-8 and hash with SHA-256 before sending.\nexport const encoding = 1;',
    '// Dates follow ISO-8601 and RFC-3339.\nexport const format = 1;',
    '// Fixed upstream in CVE-2024-1234.\nexport const patched = 1;',
    '// Step #1 reads the file.\nexport const step = 1;',
    '// Review comments arrive newest first.\nexport const order = 1;',
    '// The review step runs after tests.\nexport const reviewed = 1;',
    '// eslint-disable-next-line no-console -- per review #12\nconsole.log(1);',
    '/* oxlint-disable no-console -- see ADR 0012 */\nconsole.log(1);',
    '// @ts-expect-error -- ABC-123 tracks the upstream type.\nexport const value: string = 1;',
    '/// <reference types="node" />\nexport const node = 1;',
    '/*! Copyright 2026 Example. Licensed under MIT, see LICENSE-2 */\nexport const licensed = 1;',
    '/** @license MIT. JIRA-1 */\nexport const licensedTag = 1;',
    '// SPDX-License-Identifier: MIT\nexport const spdx = 1;',
    '// Copyright 2026 Acme. Terms in ACME-1.\nexport const notice = 1;',
    '// https://github.com/oxc-project/oxc/issues/123 still crashes the parser.\nexport const crash = 1;',
    '// Keys load from PKCS-12 bundles.\nexport const bundle = 1;',
    '// The device speaks RS-232 over USB-3.\nexport const serial = 1;',
    '// Uses IPv4 and X-509 style names.\nexport const names = 1;',
  ],
  invalid: [
    { code: '// See ADR 0012.\nexport const a = 1;', errors: [reference('ADR 0012')] },
    { code: '// Per ADR-7, keep this.\nexport const a = 1;', errors: [reference('ADR-7')] },
    { code: '// see #123\nexport const a = 1;', errors: [reference('see #123')] },
    { code: '// Added in PR #45.\nexport const a = 1;', errors: [reference('PR #45')] },
    {
      code: '// From pull request 45.\nexport const a = 1;',
      errors: [reference('pull request 45')],
    },
    { code: '// Fixes issue #9.\nexport const a = 1;', errors: [reference('issue #9')] },
    { code: '// Workaround (#310).\nexport const a = 1;', errors: [reference('(#310)')] },
    { code: '// Tracked in JIRA-123.\nexport const a = 1;', errors: [reference('JIRA-123')] },
    { code: '/* Needed for ABC-42. */\nexport const a = 1;', errors: [reference('ABC-42')] },
    { code: '// Renamed per review.\nexport const a = 1;', errors: [reference('per review')] },
    {
      code: '// See ADR 0012 about copyright handling.\nexport const a = 1;',
      errors: [reference('ADR 0012')],
    },
    {
      code: '// Licensing is tracked in LEGAL-12.\nexport const a = 1;',
      errors: [reference('LEGAL-12')],
    },
    { code: '// Blocked on PROJ-7.\nexport const a = 1;', errors: [reference('PROJ-7')] },
    {
      code: '// Changed per the code review.\nexport const a = 1;',
      errors: [reference('per the code review')],
    },
    {
      code: '// Addresses review feedback.\nexport const a = 1;',
      errors: [reference('review feedback')],
    },
    {
      code: '// As requested in review.\nexport const a = 1;',
      errors: [reference('requested in review')],
    },
    {
      code: '/**\n * Keeps the cache warm.\n * See ADR 0003 and PR #4.\n */\nexport const a = 1;',
      errors: [reference('ADR 0003')],
    },
  ],
});
