import type { Plugin } from '@oxlint/plugins';

import { helperBeforeUseRule } from './rules/helperBeforeUse.ts';
import { maxConditionChecksRule } from './rules/maxConditionChecks.ts';
import { namingConventionRule } from './rules/namingConvention.ts';
import { noAbbreviationsRule } from './rules/noAbbreviations.ts';
import { noReferenceCommentsRule } from './rules/noReferenceComments.ts';
import { requireDisableReasonRule } from './rules/requireDisableReason.ts';
import { typePlacementRule } from './rules/typePlacement.ts';

const sticklerPlugin: Plugin = {
  meta: { name: 'stickler' },
  rules: {
    'helper-before-use': helperBeforeUseRule,
    'max-condition-checks': maxConditionChecksRule,
    'naming-convention': namingConventionRule,
    'no-abbreviations': noAbbreviationsRule,
    'no-reference-comments': noReferenceCommentsRule,
    'require-disable-reason': requireDisableReasonRule,
    'type-placement': typePlacementRule,
  },
};

export default sticklerPlugin;
