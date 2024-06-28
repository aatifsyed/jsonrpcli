use std::io;

use clap::Parser;
use jsonrpcli::{Id, Request, RequestParameters, V2};
use serde_json::Value;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    id: Option<Id>,
    method: String,
    params: Vec<Value>,
}

fn main() -> serde_json::Result<()> {
    let Args { method, params, id } = Args::parse();
    serde_json::to_writer(
        io::stdout(),
        &Request {
            jsonrpc: V2,
            method,
            params: Some(RequestParameters::ByPosition(params)),
            id: Some(id.unwrap_or_default()),
        },
    )
}
