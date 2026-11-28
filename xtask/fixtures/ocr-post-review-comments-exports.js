// A pinned copy of the declarations that the OCR review workflow reads from
// alibaba/open-code-review's scripts/github-actions/post-review-comments.js.
// The workflow checks the helper out at this commit, and its tests read this
// copy, so a moved pin needs a refreshed copy.
//
// Refresh it from
// https://raw.githubusercontent.com/alibaba/open-code-review/<commit>/scripts/github-actions/post-review-comments.js
// whose sha256 is 9b6bca18055ef63afdfab396457e7641c7b521abe9488136be28e5216d038838.
//
// commit: e74f2c6bd4422bd1e12d257d1725bcce1d6b9ddd

const SUMMARY_MARKER = "<!-- ocr-summary -->";

const CHECKPOINT_VERSION = 1;
const CHECKPOINT_SHA_RE = /^[0-9a-f]{40}$/;

function runPostReviewComments() {
  throw new Error("the pinned copy only records the helper's exports");
}

function findSummaryIssueComment() {
  throw new Error("the pinned copy only records the helper's exports");
}

function readCheckpointComment() {
  throw new Error("the pinned copy only records the helper's exports");
}

// The pinned rule, verbatim: a marker only narrows the range when it is a
// complete review of this pull request, written by this version of the helper.
function validateCheckpointPayload(payload, { prNumber } = {}) {
  if (!payload) return "no payload";
  if (payload.v !== CHECKPOINT_VERSION) return `unsupported version ${JSON.stringify(payload.v)}`;
  if (prNumber != null && payload.pr !== prNumber) return `belongs to PR ${JSON.stringify(payload.pr)}`;
  if (!CHECKPOINT_SHA_RE.test(payload.head)) return "head is not a 40-hex sha";
  if (payload.terminal_state !== "complete") return `terminal_state ${JSON.stringify(payload.terminal_state)}`;
  if (typeof payload.base_ref !== "string" || payload.base_ref === "") return "missing base_ref";
  if (typeof payload.merge_base !== "string" || payload.merge_base === "") return "missing merge_base";
  if (typeof payload.fingerprint !== "string" || payload.fingerprint === "") return "missing fingerprint";
  return null;
}

module.exports = {
  runPostReviewComments,
  findSummaryIssueComment,
  readCheckpointComment,
  validateCheckpointPayload,
  SUMMARY_MARKER,
};
