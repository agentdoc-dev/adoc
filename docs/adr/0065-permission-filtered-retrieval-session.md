# ADR-0065: Permission-Filtered Retrieval Session

- Status: Accepted
- Date: 2026-09-05
- Slice: E6.1.T1

## Context

E6.1 requires authorization before retrieval candidate generation. Filtering
responses in each adapter would leave ranking and graph-derived metadata able
to reveal excluded Knowledge Objects. Explicit artifact paths must not bypass
local policy discovery.

## Decision

The core Retrieval Session assembles its graph, lexical index, and vector
index from an authorized projection. Its permission predicate consumes only
the loaded Graph Artifact and explicit policy data; it performs no I/O and
reads no clock or environment. Authored `agent_instruction` content never
grants permission.

Standalone CLI and MCP policy comes from `agentdoc.config.yaml`, including
when a caller supplies `--artifact`. The optional `retrieval_policy` block
names an explicit audience, allowed visibility classes from the existing
`public | internal | restricted` vocabulary, and excluded Object IDs. This
local configuration is trusted operator input, not authentication for a
multi-user service. Cloud must resolve current Workspace authorization and
source ACL ceilings before supplying policy to the same core predicate.

Excluded objects are absent from the search/why result corpus, including an
explicit ID lookup. No permission-denied diagnostic confirms their existence.
Invalid policy or unresolved visibility fails closed. Current artifacts
without visibility fields and without a policy keep their result corpus.
Without policy, only public/unclassified objects are permitted; internal and
restricted objects require an explicit authorized policy. This intentionally
activates enforcement for the visibility carriage shipped in E1.1. Removing
an explicit exclusion restores an otherwise-public object, not a grant to
restricted content. An explicit deny takes precedence over an allowed visibility class.

Sensitive-access delivery is implemented in E6.3. The local-CLI audit scope
is being reconciled between the product amendment and E6.3.T1 before that
bullet begins. This first tracer does not claim completed Cloud retrieval,
graph-driver closure, field redaction, or sensitive-access delivery.

## Consequences

The policy is enforced before retrieval scoring, so hidden records cannot
change ranks. Adapters load policy but do not implement permission filters.
The E6.1.T1 CLI regression covers both `search` and `why` with an explicit
artifact and proves that removing a policy exclusion restores the object.
Records whose source metadata refers to excluded IDs are withheld whole,
including their precomputed vectors. Each retained vector must also match the
current carrier kind and the existing Embedding Composition hash; stale vectors
are discarded while current vectors survive unrelated graph hash drift. Hash-covered fields and embedding inputs
are never partially scrubbed while retaining the old content hash or vector.
The closure checks complete Knowledge Object and prose block nodes, including
non-response fields that contribute to hashes. Page nodes are not retrieval
records and are outside this scan. Partial-field retrieval is E6.2 work.
`graph`, `stale`, `contradictions`, and `impacted-by` still expose unfiltered
artifact data in this tracer; E6.1.T3 closes those paths. This is not yet a
complete privacy boundary. The tool guide includes the visibility upgrade
notice and an explicit all-classes local policy recipe for valid classifications.
That recipe cannot bypass `schema.visibility_invalid`; source repair and rebuild
remain required. A CLI control pins unfiltered graph reads under an explicit
exclusion policy for T1; T3 replaces that assertion with policy exclusion.

Untrusted artifact decoder text is sanitized even without a policy: decoding
can fail before authored visibility is known. Projected reads omit artifact
warnings, which can quote denied content without an Object ID. Conditional
warning withholding is not yet whole-response presence/absence parity:
E6.1.T3 must close that diagnostic signal along with graph drivers, counts,
and timing. Adding a new conditional warning or retaining unauthenticated
warning prose by substring alone does not establish that property. When visibility
or explicit policy supplies excluded IDs, carried build errors refuse the session
with one generic `retrieval.visibility_unavailable` Error and check/build guidance;
original diagnostic details and counts are withheld. For these artifact errors,
search and why return no records and exit 2. With no excluded IDs, legacy carried diagnostics
and refusal on errors remain unchanged pending the T3 compatibility decision.
The all-public/no-policy CLI control also returns no records and exit 2 for
carried source errors; classification changes diagnostic disclosure, not the
existing refusal behavior.
Structural corruption or a carried `schema.visibility_invalid` diagnostic
still refuses the session, even without policy: missing metadata cannot be
assumed public after a producer reports classification failure. Full build
diagnostics remain available to the trusted artifact operator. Search-artifact
model mismatch and hash drift use fixed message shapes with no corpus hashes
or artifact-controlled model text, regardless of whether filtering changed data.
If some permitted vectors are stale, semantic search retains the valid vectors
and exits 0 with a warning that semantic results may be incomplete; hybrid search
can retrieve changed carriers through lexical matching. If stale bindings leave
no usable permitted vectors, semantic search refuses and hybrid falls back to
lexical search. Withheld or absent carriers alone do not cause this refusal.
Model errors retain the trusted active-provider identity. Decoder errors retain
the caller-selected artifact path and trusted reader-supplied rebuild guidance,
while suppressing artifact-controlled version values and decoder payload text.

E6.1.T2 owns typed policy errors through config parsing and response assembly;
T1 validates policy values at retrieval-session assembly. A generic config
error conversion would lose T2's required audience/policy distinction.
E6.1.T3's performance gate must measure default-deny classified repositories
as well as explicit policies. Borrowed validation and fewer repeated scans
are measured optimizations for that gate; T1 keeps the conservative projection.
