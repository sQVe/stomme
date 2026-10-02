import type { Plugin } from '@oxlint/plugins';

import { helperBeforeUseRule } from './rules/helperBeforeUse.ts';
import { maxConditionChecksRule } from './rules/maxConditionChecks.ts';
import { namingConventionRule } from './rules/namingConvention.ts';
import { noAbbreviationsRule } from './rules/noAbbreviations.ts';
import { noObjectParametersRule } from './rules/noObjectParameters.ts';
import { noReduceAccumulatorCopyRule } from './rules/noReduceAccumulatorCopy.ts';
import { noReferenceCommentsRule } from './rules/noReferenceComments.ts';
import { noReflectApplyRule } from './rules/noReflectApply.ts';
import { noReflectGetRule } from './rules/noReflectGet.ts';
import { noUnknownTypeAliasesRule } from './rules/noUnknownTypeAliases.ts';
import { noWidenThenAssertRule } from './rules/noWidenThenAssert.ts';
import { requireDisableReasonRule } from './rules/requireDisableReason.ts';
import { requireSafetyCommentForTypeAssertionRule } from './rules/requireSafetyCommentForTypeAssertion.ts';
import { typePlacementRule } from './rules/typePlacement.ts';

const sticklerPlugin: Plugin = {
  meta: { name: 'stickler' },
  rules: {
    'helper-before-use': helperBeforeUseRule,
    'max-condition-checks': maxConditionChecksRule,
    'naming-convention': namingConventionRule,
    'no-abbreviations': noAbbreviationsRule,
    'no-object-parameters': noObjectParametersRule,
    'no-reduce-accumulator-copy': noReduceAccumulatorCopyRule,
    'no-reference-comments': noReferenceCommentsRule,
    'no-reflect-apply': noReflectApplyRule,
    'no-reflect-get': noReflectGetRule,
    'no-unknown-type-aliases': noUnknownTypeAliasesRule,
    'no-widen-then-assert': noWidenThenAssertRule,
    'require-disable-reason': requireDisableReasonRule,
    'require-safety-comment-for-type-assertion': requireSafetyCommentForTypeAssertionRule,
    'type-placement': typePlacementRule,
  },
};

export default sticklerPlugin;
