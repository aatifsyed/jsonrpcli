use std::env;

use clap::{error::ErrorKind, CommandFactory, Parser};
use jsonrpc_types::{Id, RequestParameters, V2};
use serde_json::Value;
use tracing::debug;

#[allow(unused)]
mod jsonrpc_types;

const ENV_JSONRPCLI_FORCE_POSITIONAL: &str = "JSONRPCLI_FORCE_POSITIONAL";
const ENV_JSONRPCLI_FORCE_ID: &str = "JSONRPCLI_FORCE_ID";

#[derive(Parser, Debug)]
struct Args {
    /// Send a JSON-RPC notification, ignoring the response.
    ///
    /// This omits the `id` member on the request.
    #[arg(short, long)]
    notification: bool,
    /// Force the `id` member on the request (rather than passing the `null id`,
    /// which is the default behaviour).
    #[arg(short, long, env = ENV_JSONRPCLI_FORCE_ID)]
    id: Option<Id>,
    /// The (HTTP) URL to send a POST with the JSON-RPC request to.
    #[arg(short, long, env = "JSONRPCLI_URL")]
    url: String,
    method: String,
    /// Send request parameters by-name (rather than by-value, which is the
    /// default behaviour).
    ///
    /// When this argument is passed, PARAMS must be a single JSON Object.
    #[arg(short = 'N', long, env = "JSONRPCLI_NAMED")]
    named: bool,
    /// If PARAMS is empty, send an empty Array of params (rather than omitting
    /// params, which is the default behaviour).
    #[arg(short = 'p', long, env = ENV_JSONRPCLI_FORCE_POSITIONAL)]
    force_positional: bool,

    params: Vec<Value>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    debug!(?args);
    let Args {
        notification,
        url,
        method,
        named,
        force_positional,
        mut params,
        id,
    } = args;

    if named && force_positional && env::var_os(ENV_JSONRPCLI_FORCE_POSITIONAL).is_none() {
        Args::command()
            .error(
                ErrorKind::ArgumentConflict,
                "`--named` and `--force-positional` are mutually exclusive",
            )
            .exit()
    }

    let params = match (named, params.len()) {
        (false, 0) => None,
        (false, _) => Some(RequestParameters::ByPosition(params)),
        (true, 1) => match params.remove(0) {
            Value::Object(it) => Some(RequestParameters::ByName(it)),
            _ => Args::command()
                .error(
                    ErrorKind::InvalidValue,
                    "expected a JSON Object to be provided with `--named`",
                )
                .exit(),
        },
        (true, _) => Args::command()
            .error(
                ErrorKind::WrongNumberOfValues,
                "expected a single JSON Object param to be provided with `--named`",
            )
            .exit(),
    };

    let request = jsonrpc_types::Request {
        jsonrpc: V2,
        method,
        params,
        id: match (notification, id) {
            (true, None) => None,
            (false, Some(it)) => Some(it),
            (false, None) => Some(Id::Null),
            (true, Some(_)) => match env::var_os(ENV_JSONRPCLI_FORCE_ID).is_some() {
                true => None,
                false => Args::command()
                    .error(
                        ErrorKind::ArgumentConflict,
                        "`--notification` and `--id` are mutually exclusive",
                    )
                    .exit(),
            },
        },
    };

    Ok(())
}
