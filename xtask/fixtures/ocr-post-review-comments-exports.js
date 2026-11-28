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

function runPostReviewComments() {
  throw new Error("the pinned copy only records the helper's exports");
}

function findSummaryIssueComment() {
  throw new Error("the pinned copy only records the helper's exports");
}

function readCheckpointComment() {
  throw new Error("the pinned copy only records the helper's exports");
}

function validateCheckpointPayload() {
  throw new Error("the pinned copy only records the helper's exports");
}

module.exports = {
  runPostReviewComments,
  findSummaryIssueComment,
  readCheckpointComment,
  validateCheckpointPayload,
  SUMMARY_MARKER,
};
