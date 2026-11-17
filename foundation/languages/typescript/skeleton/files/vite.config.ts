import { format, lint, react, vitest } from '@sqve/stomme-ts-style';
import { defineConfig } from 'vite-plus';

export default defineConfig({
  lint: {
    extends: [lint, react, vitest],
    ignorePatterns: ['mods/**/main.js'],
    // The tsconfig uses the automatic JSX runtime, so no file imports React for JSX.
    rules: { 'react/react-in-jsx-scope': 'off' },
  },
  fmt: {
    ...format,
    // stomme sync and init write these files, and they do not follow the formatter's layout.
    ignorePatterns: [
      'pnpm-lock.yaml',
      'package.json',
      'AGENTS.md',
      'docs/stomme/**',
      'mods/**/main.js',
    ],
  },
  test: {
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx', 'mods/**/*.test.ts'],
  },
});
