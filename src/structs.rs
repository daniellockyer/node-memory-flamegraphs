#![allow(dead_code, non_snake_case)]

use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// JSON endpoint of the debugger
    #[clap(long, default_value_t = String::from("http://localhost:9229/json"))]
    pub debugger_url: String,

    /// Entry point file to profile
    #[clap(long)]
    pub entry_point: Option<String>,

    /// Frequency to sample heap (ms)
    #[clap(short, long, default_value_t = 1000)]
    pub frequency: u64,

    /// Initial delay before sampling (ms)
    #[clap(short, long, default_value_t = 0)]
    pub delay: u64,

    /// Temporary directory to store files
    #[clap(short, long, default_value_t = String::from("./.memgraphs"))]
    pub temp_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct DebuggerInstance {
    description: String,
    devtoolsFrontendUrl: String,
    devtoolsFrontendUrlCompat: String,
    faviconUrl: String,
    id: String,
    title: String,
    #[serde(rename = "type")]
    debugger_type: String,
    url: String,
    pub webSocketDebuggerUrl: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallFrame {
    pub functionName: String,
    scriptId: String,
    pub url: String,
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
#[serde(deny_unknown_fields)]
pub struct ProfileObject {
    pub head: ProfileHead,
    samples: Vec<ProfileSample>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileHead {
    pub callFrame: CallFrame,
    pub children: Vec<ProfileHead>,
    id: i64,
    pub selfSize: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, untagged)]
pub enum WebsocketResponseResult {
    Profile { profile: ProfileObject },
    Normal {},
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebsocketResponse {
    id: u64,
    pub result: WebsocketResponseResult,
}
