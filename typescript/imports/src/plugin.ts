import type { Plugin } from '@oxlint/plugins';

import { moduleBoundariesRule } from './rules/moduleBoundaries.ts';

const stommeImportsPlugin: Plugin = {
  meta: { name: 'stomme-imports' },
  rules: {
    'module-boundaries': moduleBoundariesRule,
  },
};

export default stommeImportsPlugin;
