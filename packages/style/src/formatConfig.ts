import type { OxfmtConfig } from 'vite-plus/fmt';

export const format: OxfmtConfig = {
  printWidth: 100,
  singleQuote: true,
  overrides: [{ files: ['*.md'], options: { proseWrap: 'always' } }],
  sortImports: {
    newlinesBetween: true,
    groups: ['builtin', 'external', ['parent', 'sibling', 'index'], 'style', 'unknown'],
  },
};
