# Use a single V0 core compile API

AgentDoc V0 will expose one high-level `compile_workspace()` entry point from `adoc-core`, while parser, validator, renderer, and artifact emission stay as internal modules. This keeps the first public library contract focused on the CLI's vertical workflow; lower-level APIs can be exposed later when language-server, web preview, semantic diff, or integration needs are concrete.
