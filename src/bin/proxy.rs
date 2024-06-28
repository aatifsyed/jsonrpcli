use std::io::{self, Write as _};
use std::net::SocketAddr;

use clap::Parser;
use http::Uri;
use http_body_util::{BodyExt as _, Full};
use hyper::body::{Bytes, Incoming};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use jsonrpcli::RequestParameters;
use openrpc_types::{Example, ExamplePairing, ExampleValue, ReferenceOr, SpecificationExtensions};
use std::pin::pin;
use std::time::Duration;
use tokio::net::TcpListener;

struct Config {
    remote: Uri,
}

#[derive(Parser)]
struct Args {
    local: SocketAddr,
    remote: Uri,
}

async fn proxy(
    request: http::Request<Incoming>,
    client: &Client<HttpConnector, Full<Bytes>>,
    config: &Config,
) -> anyhow::Result<http::Response<Full<Bytes>>> {
    let (mut req_parts, req_body) = request.into_parts();
    let req_body = req_body.collect().await?.to_bytes();

    req_parts.uri.clone_from(&config.remote);

    let response = client
        .request(http::Request::from_parts(
            req_parts,
            Full::new(req_body.clone()),
        ))
        .await?;

    let (resp_parts, resp_body) = response.into_parts();
    let resp_body = resp_body.collect().await?.to_bytes();

    if let (
        Ok(jsonrpcli::Request {
            jsonrpc: _,
            method,
            params,
            id: _,
        }),
        Ok(jsonrpcli::Response {
            jsonrpc: _,
            result: Ok(result),
            id: _,
        }),
    ) = (
        serde_json::from_slice(&req_body),
        serde_json::from_slice(&resp_body),
    ) {
        let pairing = ExamplePairing {
            name: method,
            description: None,
            summary: None,
            params: match params {
                Some(params) => match params {
                    RequestParameters::ByPosition(it) => it
                        .into_iter()
                        .map(|it| {
                            ReferenceOr::Item(Example {
                                name: None,
                                summary: None,
                                description: None,
                                value: ExampleValue::Embedded(it),
                                extensions: SpecificationExtensions::default(),
                            })
                        })
                        .collect(),
                    RequestParameters::ByName(it) => it
                        .into_iter()
                        .map(|(name, value)| {
                            ReferenceOr::Item(Example {
                                name: Some(name),
                                summary: None,
                                description: None,
                                value: ExampleValue::Embedded(value),
                                extensions: SpecificationExtensions::default(),
                            })
                        })
                        .collect(),
                },
                None => vec![],
            },
            result: Some(ReferenceOr::Item(Example {
                name: None,
                summary: None,
                description: None,
                value: ExampleValue::Embedded(result),
                extensions: SpecificationExtensions::default(),
            })),
            extensions: SpecificationExtensions::default(),
        };
        let mut stdout = io::stdout().lock();
        let _ = serde_json::to_writer(&mut stdout, &pairing);
        let _ = writeln!(stdout);
    }

    Ok(http::Response::from_parts(resp_parts, Full::new(resp_body)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    _main().await
}

async fn _main() -> anyhow::Result<()> {
    let Args { local, remote } = Args::parse();
    let client = &*Box::leak(Box::new(
        Client::builder(hyper_util::rt::TokioExecutor::new())
            .build::<_, Full<Bytes>>(HttpConnector::new()),
    ));

    let config = &*Box::leak(Box::new(Config { remote }));

    let listener = TcpListener::bind(local).await?;

    let server = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut ctrl_c = pin!(tokio::signal::ctrl_c());

    loop {
        tokio::select! {
            conn = listener.accept() => {
                let (stream, peer_addr) = match conn {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("accept error: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };
                eprintln!("incomming connection accepted: {}", peer_addr);

                let stream = hyper_util::rt::TokioIo::new(Box::pin(stream));

                let conn = server.serve_connection_with_upgrades(stream, hyper::service::service_fn(|it|proxy(it, client, config)));

                let conn = graceful.watch(conn.into_owned());

                tokio::spawn(async move {
                    if let Err(err) = conn.await {
                        eprintln!("connection error: {}", err);
                    }
                    eprintln!("connection dropped: {}", peer_addr);
                });
            },

            _ = ctrl_c.as_mut() => {
                drop(listener);
                eprintln!("Ctrl-C received, starting shutdown");
                    break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            eprintln!("Gracefully shutdown!");
        },
        _ = tokio::time::sleep(Duration::from_secs(10)) => {
            eprintln!("Waited 10 seconds for graceful shutdown, aborting...");
        }
    }

    Ok(())
}
