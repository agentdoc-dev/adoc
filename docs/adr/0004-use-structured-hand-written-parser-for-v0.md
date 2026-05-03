# Use a structured hand-written parser for V0

AgentDoc V0 will use a structured hand-written, line-oriented parser instead of starting with a parser generator or combinator framework. The grammar is intentionally block-oriented in V0, and the product needs tailored diagnostics, source spans, and error recovery; the parser should still expose clean AST and diagnostic boundaries so internals can be replaced later if nested blocks, richer inline syntax, or complex recovery justify it.
