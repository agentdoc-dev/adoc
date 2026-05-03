# Use `adoc` as the CLI command

AgentDoc's local CLI will use the `adoc` command instead of `agentdoc`. This keeps common workflows short (`adoc check`, `adoc build`) while the product name remains AgentDoc; the trade-off is that `.adoc` also names the source file extension and is commonly associated with AsciiDoc, so this decision is recorded explicitly.
