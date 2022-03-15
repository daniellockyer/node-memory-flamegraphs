#![allow(dead_code, non_snake_case)]

use std::{thread, time};

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[derive(Debug, Deserialize)]
struct DebuggerInstance {
    description: String,
    devtoolsFrontendUrl: String,
    devtoolsFrontendUrlCompat: String,
    faviconUrl: String,
    id: String,
    title: String,
    #[serde(rename = "type")]
    debugger_type: String,
    url: String,
    webSocketDebuggerUrl: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CallFrame {
    functionName: String,
    scriptId: String,
    url: String,
    columnNumber: i64,
    lineNumber: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileSample {
    size: i64,
    nodeId: i64,
    ordinal: i64,
}

#[derive(Debug, Deserialize)]
//#[serde(deny_unknown_fields)]
struct ProfileObject {
    head: ProfileHead,
    //samples: Vec<ProfileSample>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileHead {
    callFrame: CallFrame,
    children: Vec<ProfileHead>,
    id: i64,
    selfSize: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, untagged)]
enum WebsocketResponseResult {
    Profile { profile: ProfileObject },
    Normal {},
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WebsocketResponse {
    id: u64,
    result: WebsocketResponseResult,
}

fn process(profile: ProfileHead, root: String) {
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
    let body: Vec<DebuggerInstance> = reqwest::get("http://localhost:9229/json")
        .await?
        .json()
        .await?;

    if body.is_empty() {
        println!("No debuggers could be found");
        return Ok(());
    }

    let debugger_url = &body[0].webSocketDebuggerUrl;

    let (ws_stream, _) = connect_async(debugger_url).await?;
    let (mut tx, rx) = ws_stream.split();

    tokio::spawn(async move {
        rx.for_each(|message| async {
            let data = message.unwrap().into_data();

            let mut deserializer = serde_json::Deserializer::from_slice(&data);
            deserializer.disable_recursion_limit();

            let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
            let v = WebsocketResponse::deserialize(deserializer).unwrap();

            if let WebsocketResponseResult::Profile { profile } = v.result {
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

    let sleep_delay = time::Duration::from_millis(1000);

    loop {
        tx.send(Message::Text(
            json!({"id": 1, "method": "HeapProfiler.getSamplingProfile"}).to_string(),
        ))
        .await?;
        thread::sleep(sleep_delay);
    }

    Ok(())
}
