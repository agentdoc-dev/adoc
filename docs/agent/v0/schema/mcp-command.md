# AgentDoc MCP Command Schema

The V2.2 MCP command envelope is `adoc.mcp.command.v0`.

Command envelopes wrap CLI-equivalent MCP tool results that have an exit code. The command envelope reports schema version, command name, `ok`, exit code, and the command-specific result.

`adoc_project_status` returns its own `adoc.project.status.v0` envelope because it is the readiness contract, not a CLI command echo.
