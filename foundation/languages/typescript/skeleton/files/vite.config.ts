import { format, lint, react, vitest } from '@sqve/stomme-ts-style';
import { defineConfig } from 'vite-plus';

export default defineConfig({
  lint: {
    extends: [lint, react, vitest],
    ignorePatterns: ['mods/**/main.js'],
  },
  fmt: {
    ...format,
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
