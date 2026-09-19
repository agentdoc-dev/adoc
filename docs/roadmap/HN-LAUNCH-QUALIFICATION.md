# Extended launch qualification

Scope update, 2026-09-19: further accessibility qualification is no longer a
launch gate, per the user. Retain the verified checkbox fix and completed
browser evidence. Do not enable VoiceOver.


Status: all nine slices planned; implementation explicitly authorized 2026-09-18.
Cargo installation, Ubuntu VM installation, performance measurements and the real
Claude Desktop UI journey have passed their recorded checks. Native final-candidate
CI remains in progress. Security review, its repair checks and the coverage/disposition
report are complete within the documented engineering scope.
Baseline: `ce836c1c1ba94a579d3b022f9cb3337ae06ece0a`, branch
`feat/hn-launch-readiness`. The preceding five implementation slices are complete.
Authority: [HN-9 through HN-17](../product/HN-LAUNCH-READINESS.md#extended-qualification-requested-2026-09-18).

The user chose engineering performance evidence and accessibility testing, not
formal certification. Full requested coverage remains in scope; unavailable
machines or checks are explicit pending items, never successful substitutes.

## Sequence and dependencies

| Slice | Observable outcome | Depends on | Recorded status, 2026-09-19 |
|---|---|---|---|
| [HN-M4.T1](../plans/HN-M4.T1.md) | Cargo-installed CLI/MCP completes the first-use journey | Completed M1-M3 | Passed: isolated release-profile CLI/MCP installs and real-model smoke |
| HN-M4.T2 | Native Intel macOS package runs successfully | T1; candidate available to runner | Running: native Intel runner compiling pinned ONNX Runtime |
| HN-M4.T3 | Windows runtime qualification with explicit failure/support disposition | T1 | Passed: native Windows installation and runtime job 35434373133 |
| HN-M4.T4 | Reproducible installation on a pristine OS | T1; target-specific fixes | Passed: fresh Ubuntu ARM64 VM; offline runtime checked; pristine macOS/Windows untested |
| HN-M5.T1 | Full current-code and reachable-history security coverage | Frozen candidate | Completed engineering review: scans triaged, repairs tested, residual limits documented |
| HN-M5.T2 | Reproducible performance baseline with correctness evidence | T1; freeze any security fixes | Passed: three corpus sizes, correctness checks and retained raw samples |
| HN-M5.T3 | HTML/CLI accessibility evidence and repaired blockers | T1; representative rendered fixtures | Closed by user: further accessibility qualification removed; existing fix retained |
| HN-M6.T1 | Real desktop MCP UI configuration and cited answer | T1 | Passed: Claude Desktop UI status and cited answer; temporary configuration removed |
| HN-M6.T2 | Real fork PR proves contributor CI boundaries | Candidate workflows available as PR base | Draft PR #259 running: required CI passed, Intel qualification pending |

Implementation is underway. M5.T1 runs against recorded source snapshots;
subsequent repairs invalidate affected qualification evidence.
T2/T3 require hosted execution; T4 must not reuse a preloaded hosted-runner result
as proof of a pristine OS. M6.T2 must target the candidate's workflows, not silently
exercise old main. Final closure requires every row passed or visibly unresolved.

## HN-M4: Installation and native runtime

### HN-M4.T1: Cargo release-profile installation

HN-9 plus HN-2/3/4. Use the [detailed plan](../plans/HN-M4.T1.md).
Install both path crates at one immutable revision into a temporary root, with a
fresh compilation directory; exercise the installed paths, not target/debug or
an existing PATH entry. Record default release profile and real-model behavior.
No global install, package-registry publication or new dependency required.

### HN-M4.T2: Native Intel macOS runtime

HN-10. Own `.github/workflows/release.yml`, installation matrix and evidence only.
Reuse its existing native matrix and `scripts/smoke-test.py --embeddings` against
extracted CLI/MCP archives. Record candidate SHA, runner image/version, native
`rustc -vV` host, CPU architecture, binary architecture, checksum, modes and logs.
Require x86_64-apple-darwin execution; ARM cross-compilation or Rosetta-only results
are supplementary, not native qualification. Include paths containing spaces and
Unicode, read-only source failure, process termination and repeat initialization.

Use a non-publishing workflow invocation. `workflow_dispatch` availability depends
on the workflow existing on the default branch; otherwise use the approved PR
validation path. Do not push a release tag merely to run tests. Reuse the other
matrix results for Linux/macOS ARM evidence if they actually run. A timeout or
failed model download remains a failure with an explicit recovery retest.

### HN-M4.T3: Native Windows compatibility

HN-11. Own a qualification-only Windows job and the smallest portable smoke fix.
Start with a native Windows/MSVC build at the locked toolchain. The current smoke
checks literal `adoc`/`adoc-mcp` file names; write a failing platform-path check,
then use `.exe` only on Windows without changing the POSIX path. Retain real
subprocess/MCP behavior; do not stub away process or embedding failures.

Exercise install/launch, Unicode and spaced paths, CRLF input, source citations,
config discovery, lexical and actual semantic retrieval, repeat init, invalid
source, stdio and child cleanup. Record native dependencies and targeted failures
from Unix-only assumptions. Do not replace an unsupported dependency or redesign
path handling speculatively. A demonstrated defect gets a bounded repair and
regression check. Public Windows support requires the entire intended journey to
pass; a successful compile alone does not qualify it. No Windows release promise
or artifact is added before evidence exists.

### HN-M4.T4: Pristine OS installation

HN-12. Own an environment recipe and installation evidence, not the user's OS.
Start with a clean Linux OS VM created from a verified official image, and a
separate clean macOS environment when available. Record each tested OS explicitly;
a Linux pass does not imply pristine Windows/macOS installation. No shared host
HOME, Cargo registry/target, model cache, compiler cache, credentials or mounted
checkout. Copy/clone only the candidate and documented fixture; install only
listed prerequisites. Record initial package inventory, image identity, exact
bootstrap transcript, network requirements and first model download.

Run source build and Cargo installation, then both smoke modes. Repeat the offline
path after disabling network at the isolated environment boundary; use the
already-installed binary and do not equate dependency bootstrap with offline
runtime. Preserve user host settings. A container proves clean userspace only;
label it accurately and retain the VM/OS check as pending. VM access or software
licenses not available locally are explicit environment dependencies.

## HN-M5: Security, performance and accessibility

### HN-M5.T1: Current-code and reachable-history security review

HN-13. Freeze candidate, enumerate tracked files and refs, then create a coverage
ledger before review. The local clone is non-shallow (1,307 reachable commits at
planning time); this is not proof that every remote/deleted ref was fetched.
Record remote heads/tags and fetched objects, submodules/LFS if present, missing
refs and scan limits. Scan all reachable history for secrets and unsafe artifacts;
never print token values or upload the repository to a new service.

Review all first-party current code/build/workflow surfaces with file-level
coverage, including four crates, scripts, dependency/build configuration and
release execution. Prioritize parser/resource limits, URL/HTML escaping, Git
subprocess arguments, path traversal/symlinks, migration/patch atomicity and stale
hashes, policy/visibility boundaries, untrusted graph/search input, MCP trust and
writes, model downloads, gateway audit egress, and privileged workflow execution.
Trace callers and negative tests; inspect security-relevant historical changes
and any history-scan findings manually. Dependency advisories and license/source
policy are distinct checks. Pin scanner versions/configuration and record coverage
and false-positive disposition; missing scanners are not clean results.

Evidence: redacted findings with severity, preconditions, reproduction, affected
versions and owning files; complete current-file coverage; reachable-history
scan manifest; remediation/retest ledger. Do not call automated scans an exhaustive
manual review of every historic revision or third-party dependency source. If
that level is needed, expand scope explicitly rather than claiming it happened.
Never use found credentials, test external victims, rotate credentials, rewrite
history or publish vulnerability details as an incidental audit step. Escalate
real exposed secrets privately with redacted location evidence.

### HN-M5.T2: Performance evidence

HN-14. Own a stdlib benchmark driver/data recipe and a report; reuse CLI/MCP and
existing pilots. Fix candidate, seed, model identity, hardware, toolchain and
power/load conditions. Use small, medium and large documented corpora, preserving
known IDs/relations/citations so every measured command also checks correctness.
Measure compile/check, no-embedding build, first and cached embedding build,
lexical/hybrid/semantic search, and MCP startup/request latency. Separate model
network download, cold process, warm cache and steady-state work.

Record raw repetitions, median/range, peak memory using OS-appropriate facilities,
input sizes and failures. Use sufficient repeated samples and describe small
sample limits; do not claim p99 from a handful of runs. Define dataset/time/memory
ceilings to prevent the benchmark exhausting the host. No performance claim or
regression gate is invented before a baseline and intended workload exist. Profile
only demonstrated bottlenecks; rerun the same dataset after any repair.

### HN-M5.T3: Accessibility engineering evaluation

HN-15. Own representative HTML/CLI fixtures, renderer/CLI fixes only for reproduced
defects, and a scoped report. Evaluate generated content against applicable WCAG
2.2 A/AA criteria: document language/title, headings/landmarks, links, lists/tables,
image alternatives, focus/keyboard access, zoom/reflow and contrast. Include prose,
typed objects, relation links, quarantined markup and audience-restricted output.
Retain escaping and visibility policy through any semantic HTML change.

Combine automated checking with actual keyboard navigation. The user explicitly
excluded VoiceOver on 2026-09-18; do not enable it. Screen-reader runtime testing
is outside the accepted scope. Inspect CLI plain
output, NO_COLOR, errors and citations separately; a CLI accessibility check is
not a WCAG certification. Record criteria as pass/fail/not-applicable/not-tested,
with reasons. A scanner-only pass, generated screenshot, or inaccessible screen
reader environment cannot establish manual coverage. Include user testing where
available, and disclose when it is absent. No blanket conformance claim from a
sample. See [W3C evaluation guidance](https://www.w3.org/WAI/test-evaluate/).

## HN-M6: Real user/client and contributor journeys

### HN-M6.T1: Named desktop MCP client UI

HN-16. Own a Claude Desktop setup guide section and redacted evidence. Inspect
installed client/version and supported local MCP UI first; confirm current vendor
docs during execution. Use T1's installed gateway, an absolute executable path,
and a disposable public-only project. Preserve existing client configuration and
server entries; record the minimal addition and reversible cleanup.

Through the real UI, add/enable the local server, inspect connection/tool status,
then ask for project status and `billing.refund-window` with explicit project_root.
Verify visible tool results and `docs/index.adoc:3:1`, plus disabled patch application.
Headless Claude Code calls are useful prior evidence, not this acceptance test.
Use only the synthetic fixture in the third-party chat. If sign-in, permission,
unsupported UI, or unavailable computer control blocks progress, record the exact
step and request the necessary user interaction; never claim a simulated UI pass.

### HN-M6.T2: Live human fork PR

HN-17. Own one clearly labeled draft validation PR and its evidence. User explicitly
requested a live fork PR; keep it minimal, no merges/releases or unrelated messages.
Prepare exact base/head branches and reviewable harmless diff first. Base must
include the candidate workflow changes. Use the authenticated human's fork, not
an invented identity or token. Record whether that account is first-time or
trusted: the maintainer's own fork cannot establish first-time approval behavior.

Observe the actual pull_request event, head/base/merge SHA, ordinary required `ci`
gate, secret-backed Claude job intentionally skipped, and documented Action
behavior. Do not print secrets to prove absence or enable write tokens/secrets for
forks. If approval is required, preserve that state and obtain the authorized
maintainer action without weakening repository policy. Recheck after a harmless
synchronize event if needed. Read checks and expected review results; no empty
inbox or cancelled job is counted as success. Leave the draft PR clearly identified
and report cleanup state; do not merge or delete the user's fork automatically.

## Delivery and review policy

Plan stages do not claim execution or certification. Every test record includes
candidate/source SHA, binary digest, tool/OS versions, command/interaction, result
and retained evidence. Store raw sensitive logs locally under `.git/agentic-workflow/`;
commit sanitized reproducible recipes/reports only. Existing user work is preserved.

Use one commit per completed implementation slice. Behavior repairs start with a
failing regression check, then targeted tests plus affected repository gates.
Documentation/evidence-only changes use link/diff checks; broad tests are reused
only with recorded unchanged code/environment scope. Qualifying a new OS needs
real native execution, regardless of local green checks.

Coordinator owns architecture, scope, Git and remote actions. At most two isolated
workers: aw-builder gpt-5.6-terra/medium (24 turns) for a bounded repair; independent
aw-refuter gpt-5.6-sol/high and real Claude Opus/high (16 turns each). These models
were successfully used in the initial implementation; recheck availability before
dispatch and record substitutions. Security review is divided by owned threat
surfaces/file manifests, never squeezed into one 16-turn all-repository claim.
Workers receive only scoped code/evidence; no recursive delegation or global
plugin changes. Two repair rounds, then diagnose unresolved findings.

Final reviewers assess correctness/security and evidence validity respectively.
Incomplete tools, platform access or UI testing stay pending. Re-run the affected
matrix when fixes alter binaries, packaging or trust boundaries. Update the
[launch verification report](../guides/launch-readiness.md) only with actual results.

Primary command/workflow references (checked 2026-09-18):
[Cargo install](https://doc.rust-lang.org/cargo/commands/cargo-install.html),
[Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html),
[GitHub workflow events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows).

## Executable plans

- [HN-M4.T1](../plans/HN-M4.T1.md)
- [HN-M4.T2](../plans/HN-M4.T2.md)
- [HN-M4.T3](../plans/HN-M4.T3.md)
- [HN-M4.T4](../plans/HN-M4.T4.md)
- [HN-M5.T1](../plans/HN-M5.T1.md)
- [HN-M5.T2](../plans/HN-M5.T2.md)
- [HN-M5.T3](../plans/HN-M5.T3.md)
- [HN-M6.T1](../plans/HN-M6.T1.md)
- [HN-M6.T2](../plans/HN-M6.T2.md)
