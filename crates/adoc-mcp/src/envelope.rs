use serde::Serialize;

pub const COMMAND_SCHEMA_VERSION: &str = "adoc.mcp.command.v0";

#[derive(Debug, Clone, Serialize)]
pub struct CommandEnvelope<T>
where
    T: Serialize,
{
    pub schema_version: &'static str,
    pub command: &'static str,
    pub ok: bool,
    pub exit_code: i32,
    pub result: T,
}

pub fn command_envelope<T>(command: &'static str, exit_code: i32, result: T) -> CommandEnvelope<T>
where
    T: Serialize,
{
    CommandEnvelope {
        schema_version: COMMAND_SCHEMA_VERSION,
        command,
        ok: exit_code == 0,
        exit_code,
        result,
    }
}
