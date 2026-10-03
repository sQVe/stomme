import { RuleTester } from 'oxlint/plugins-dev';
import { describe, it } from 'vitest';

RuleTester.describe = describe;
RuleTester.it = it;

// Rule tests run in process; tests/packageSuite.ts covers loading the built plugin as a consumer.
export const ruleTester = new RuleTester({
  languageOptions: { sourceType: 'module', parserOptions: { lang: 'ts' } },
});

export const jsxRuleTester = new RuleTester({
  languageOptions: { sourceType: 'module', parserOptions: { lang: 'tsx' } },
});
