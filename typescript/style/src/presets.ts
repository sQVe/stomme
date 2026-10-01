import type { OxlintConfig } from 'vite-plus/lint';

import { testFiles } from './lintConfig.ts';

export const react: OxlintConfig = {
  plugins: ['react'],
  rules: {
    'react/rules-of-hooks': 'error',
    'react/exhaustive-deps': 'error',
  },
};

export const vitest: OxlintConfig = {
  plugins: ['vitest'],
  overrides: [
    {
      files: testFiles,
      rules: {
        // Vitest prints a second argument with the failure, such as the fix for a failed check.
        'vitest/valid-expect': ['error', { maxArgs: 2 }],
      },
    },
  ],
};
