#![allow(dead_code, non_snake_case)]

use std::{thread, time};

use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

mod structs;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// JSON endpoint of the debugger
    #[clap(short, long, default_value_t=String::from("http://localhost:9229/json"))]
    debugger_url: String,

    /// Frequency to sample heap (ms)
    #[clap(short, long, default_value_t = 1000)]
    frequency: u64,

    /// Initial delay before sampling (ms)
    #[clap(short, long, default_value_t = 0)]
    initial_delay: u64,
}

fn process(profile: structs::ProfileHead, root: String) {
    let stack = format!(
        "{};{} {}",
        root,
        profile.callFrame.functionName.to_owned(),
        profile.callFrame.url.to_owned(),
    );

    if profile.callFrame.functionName != "" {
        println!("{} {}", stack, profile.selfSize);
    }

    for p in profile.children {
        process(p, stack.to_owned());
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let body: Vec<structs::DebuggerInstance> =
        reqwest::get(args.debugger_url).await?.json().await?;

    if body.is_empty() {
        println!("No debuggers could be found");
        return Ok(());
    }

    let (ws_stream, _) = connect_async(&body[0].webSocketDebuggerUrl).await?;
    let (mut tx, rx) = ws_stream.split();

    tokio::spawn(async move {
        rx.for_each(|message| async {
            let data = message.unwrap().into_data();

            let mut deserializer = serde_json::Deserializer::from_slice(&data);
            deserializer.disable_recursion_limit();

            let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
            let v = structs::WebsocketResponse::deserialize(deserializer).unwrap();

            if let structs::WebsocketResponseResult::Profile { profile } = v.result {
                process(profile.head, "".to_string());
            }
        })
        .await;
    });

    tx.send(Message::Text(
        json!({"id": 0, "method": "Runtime.runIfWaitingForDebugger"}).to_string(),
    ))
    .await?;

    tx.send(Message::Text(
        json!({"id": 0, "method": "HeapProfiler.startSampling"}).to_string(),
    ))
    .await?;

    let sleep_delay = time::Duration::from_millis(args.frequency);

    thread::sleep(time::Duration::from_millis(args.initial_delay));

    loop {
        tx.send(Message::Text(
            json!({"id": 1, "method": "HeapProfiler.getSamplingProfile"}).to_string(),
        ))
        .await?;
        thread::sleep(sleep_delay);
    }
}
