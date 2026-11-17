import { packageVersion } from './packageVersion.ts';

export const tauModuleConfig = `schema = 1
version = "${packageVersion}"
language = "typescript"

[modules.checkRecord]
entry = "src/checkRecord/checkRecord.ts"
public = ["src/checkRecord/identity.ts", "src/checkRecord/reuseDecision.ts"]
imports = ["errors", "git", "record"]

[modules.codemode]
entry = "src/codemode/codemode.ts"
public = ["src/codemode/controlTools.ts", "src/codemode/fullOutput.ts", "src/codemode/guidelines.ts"]

[modules.command]
entry = "src/command.ts"
imports = ["schema"]

[modules.config]
entry = "src/config/config.ts"
public = ["src/config/models.ts"]
imports = ["errors", "schema"]

[modules.dialog]
entry = "src/dialog/dialog.ts"
public = ["src/dialog/blockedStatus.ts", "src/dialog/box.ts", "src/dialog/keys.ts"]
imports = ["worker"]

[modules.errors]
entry = "src/errors.ts"

[modules.fileLock]
entry = "src/fileLock/fileLock.ts"
imports = ["errors", "process", "schema"]

[modules.git]
entry = "src/git.ts"
imports = ["errors"]

[modules.github]
entry = "src/github/github.ts"
public = ["src/github/repositoryTarget.ts", "src/github/repositoryTargetDecisions.ts"]
imports = ["command", "errors", "git", "schema"]

[modules.hc]
entry = "src/hc/hc.ts"
public = ["src/hc/connect.ts", "src/hc/events.ts"]
imports = ["command", "errors", "schema"]

[modules.improvement]
entry = "src/improvement/improvement.ts"
public = ["src/improvement/event.ts", "src/improvement/projectRoot.ts", "src/improvement/transcript.ts", "src/improvement/workerRecord.ts"]
imports = ["errors", "git", "record", "schema", "tauDirectory"]

[modules.inbox]
entry = "src/inbox/inbox.ts"
public = ["src/inbox/receiptDecision.ts"]
imports = ["errors", "fileLock", "hc", "record", "schema", "wake"]

[modules.instructions]
entry = "src/instructions/instructions.ts"
public = ["src/instructions/systemPrompt.ts", "src/instructions/typescriptRules.ts"]

[modules.linear]
entry = "src/linear/linear.ts"
public = ["src/linear/descriptionMatch.ts", "src/linear/ownership.ts"]
imports = ["command", "errors"]

[modules.process]
entry = "src/process.ts"
imports = ["errors"]

[modules.record]
entry = "src/record.ts"
imports = ["errors", "schema", "tauDirectory"]

[modules.reviewBots]
entry = "src/reviewBots/reviewBots.ts"
imports = ["command", "config", "github", "schema"]

[modules.reviewCapture]
entry = "src/reviewCapture/reviewCapture.ts"
public = ["src/reviewCapture/evidence.ts", "src/reviewCapture/reports.ts"]
imports = ["command", "errors", "git", "github", "record", "schema", "skillTools", "tauDirectory"]

[modules.schema]
entry = "src/schema.ts"

[modules.shell]
entry = "src/shell.ts"

[modules.skillTools]
entry = "src/skillTools.ts"
imports = ["github"]

[modules.tauDirectory]
entry = "src/tauDirectory.ts"
imports = ["errors", "git"]

[modules.wake]
entry = "src/wake/wake.ts"

[modules.worker]
entry = "src/worker.ts"
imports = ["schema"]

[modules."skills/tdd"]
entry = "src/skills/tdd/tdd.ts"

[modules."extensions/coding"]
entry = "src/extensions/coding.ts"

[modules."extensions/commit"]
entry = "src/extensions/commit/commit.ts"

[modules."extensions/probe"]
entry = "src/extensions/probe/probe.ts"
imports = ["config", "errors"]

[modules."extensions/flat"]
entry = "src/extensions/flat.ts"
imports = ["errors"]

[modules."extensions/snippets"]
entry = "src/extensions/snippets/snippets.ts"
imports = ["skillTools"]

[modules."extensions/inbox"]
entry = "src/extensions/inbox/inbox.ts"
imports = ["inbox"]

[modules."extensions/subagents"]
entry = "src/extensions/subagents/subagents.ts"
imports = ["extensions/subagents/controller"]

[modules."extensions/subagents/controller"]
entry = "src/extensions/subagents/controller/controller.ts"
public = ["src/extensions/subagents/controller/budget.ts", "src/extensions/subagents/controller/record.ts"]
imports = ["extensions/subagents"]

[modules."extensions/tdd"]
entry = "src/extensions/tdd/tdd.ts"
imports = ["extensions/tdd/runner"]

[modules."extensions/tdd/runner"]
entry = "src/extensions/tdd/runner/runner.ts"
public = ["src/extensions/tdd/runner/constants.ts", "src/extensions/tdd/runner/report.ts", "src/extensions/tdd/runner/retention.ts", "src/extensions/tdd/runner/runners.ts", "src/extensions/tdd/runner/types.ts", "src/extensions/tdd/runner/vitest.ts"]

[modules.tau]
entry = "src/tau.ts"
imports = [
  "checkRecord",
  "codemode",
  "command",
  "config",
  "dialog",
  "errors",
  "fileLock",
  "git",
  "github",
  "hc",
  "improvement",
  "inbox",
  "instructions",
  "linear",
  "process",
  "record",
  "reviewBots",
  "reviewCapture",
  "schema",
  "shell",
  "skillTools",
  "skills/tdd",
  "tauDirectory",
  "wake",
  "worker",
  "extensions/coding",
  "extensions/commit",
  "extensions/flat",
  "extensions/inbox",
  "extensions/probe",
  "extensions/snippets",
  "extensions/subagents",
  "extensions/subagents/controller",
  "extensions/tdd",
  "extensions/tdd/runner",
]
`;
