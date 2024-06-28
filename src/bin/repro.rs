use std::io;

use anyhow::bail;
use clap::Parser;
use jsonrpcli::{RequestParameters, V2};
use openrpc_types::{resolved::ExamplePairing, Example, ExampleValue};

#[derive(Parser)]
struct Args {
    url: String,
}

fn main() -> anyhow::Result<()> {
    let Args { url } = Args::parse();
    for it in serde_json::Deserializer::from_reader(io::stdin()).into_iter::<ExamplePairing>() {
        if let ExamplePairing {
            name: method_name,
            params,
            result:
                Some(Example {
                    value: ExampleValue::Embedded(expected_result),
                    ..
                }),
            ..
        } = it?
        {
            let response = ureq::post(&url)
                .send_json(jsonrpcli::Request {
                    jsonrpc: V2,
                    method: method_name.clone(),
                    params: Some(RequestParameters::ByPosition(
                        params
                            .into_iter()
                            .map(|example| match example.value {
                                ExampleValue::External(_) => {
                                    bail!("unexpected external example value")
                                }
                                ExampleValue::Embedded(it) => Ok(it),
                            })
                            .collect::<Result<_, _>>()?,
                    )),
                    id: Some(jsonrpcli::Id::Null),
                })?
                .into_json::<jsonrpcli::Response>()?;
            match response.result {
                Ok(actual_result) => match expected_result == actual_result {
                    true => {}
                    false => eprintln!("mismatch for {}", method_name),
                },
                Err(e) => bail!("error for {}: {}", method_name, e.message),
            }
        };
    }
    Ok(())
}
