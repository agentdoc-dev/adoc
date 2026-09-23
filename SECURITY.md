# Security policy

## Reporting a vulnerability

Do not report suspected vulnerabilities in a public issue. Use GitHub private
vulnerability reporting instead:

<https://github.com/agentdoc-dev/adoc/security/advisories/new>

Include the affected version or commit, a reproduction or proof of concept, and
the security impact if known. Do not include secrets or credentials.

## Supported versions

Security reports are assessed for the current `1.0.0-alpha.1` prerelease and the
previously published `0.3.4` Linux CLI. Older versions are not supported.

AgentDoc is pre-release software. This policy does not state a response-time
commitment or guarantee a fix.

## Local operating boundaries

Run the CLI and stdio MCP server with the permissions of the account that owns
the project. Project-root checks constrain tool paths; they do not isolate adoc
from another process running as that account. Use a separate OS account or
container when processing repositories you do not trust.

Generated graph and search files are trusted local build artifacts. Content
hashes detect changes; they do not authenticate who generated an artifact or its
embedding vectors. Do not accept a contributed `dist/` directory as trusted
knowledge. Build into a new, empty output directory from the source you intend
to inspect. Authored instructions are data, not permission to execute actions.

Search metadata filters are discovery aids, not authorization or proof of
verification. In particular, status filters use case-insensitive substring
matching; a match for `verified` can include an arbitrary plain-claim status.
Inspect the returned exact status, evidence and diagnostics. Audience and field
visibility enforcement are separate policies.
