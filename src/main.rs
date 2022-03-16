#[macro_use]
extern crate log;

use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    thread,
    time::{self, SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use env_logger::Env;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use subprocess::{Popen, PopenConfig};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

mod structs;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// JSON endpoint of the debugger
    #[clap(long, default_value_t = String::from("http://localhost:9229/json"))]
    debugger_url: String,

    /// Entry point file to profile
    #[clap(long)]
    entry_point: Option<String>,

    /// Frequency to sample heap (ms)
    #[clap(short, long, default_value_t = 1000)]
    frequency: u64,

    /// Initial delay before sampling (ms)
    #[clap(short, long, default_value_t = 0)]
    delay: u64,

    /// Temporary directory to store files
    #[clap(short, long, default_value_t = String::from("./.memgraphs"))]
    temp_dir: String,
}

fn process<W: Write>(writer: &mut W, profile: structs::ProfileHead, root: String) {
    let stack = format!(
        "{};{} {}",
        root,
        profile.callFrame.functionName.to_owned(),
        profile.callFrame.url.to_owned(),
    );

    let output = format!("{} {}\n", stack, profile.selfSize);
    writer
        .write_all(output.as_bytes())
        .expect("Could not write to file");

    for p in profile.children {
        process(writer, p, stack.to_owned());
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = Env::default().filter_or("LOG_LEVEL", "info");
    env_logger::init_from_env(env);

    let args = Args::parse();

    let temp_dir_path = args.temp_dir.to_owned();
    let temp_dir = Path::new(&temp_dir_path);

    if temp_dir.exists() {
        fs::remove_dir_all(temp_dir)?;
    }

    fs::create_dir(temp_dir)?;

    if let Some(entry_point) = args.entry_point {
        info!("Launching `{}`", entry_point);

        let entry_cwd = fs::canonicalize(&entry_point)?;
        let entry_cwd_parent = entry_cwd.parent().ok_or(".")?.as_os_str();

        Popen::create(
            &["node", "--inspect-brk", &entry_point],
            PopenConfig {
                detached: true,
                cwd: Some(entry_cwd_parent.into()),
                ..Default::default()
            },
        )?;
        thread::sleep(time::Duration::from_millis(500));
    }

    info!("Fetching debugger JSON via {}", args.debugger_url);

    let body: Vec<structs::DebuggerInstance> =
        reqwest::get(args.debugger_url).await?.json().await?;

    if body.is_empty() {
        error!("No debuggers could be found");
        return Ok(());
    }

    info!("Connecting to {}", &body[0].webSocketDebuggerUrl);
    let (ws_stream, _) = connect_async(&body[0].webSocketDebuggerUrl).await?;
    let (mut tx, rx) = ws_stream.split();
    info!("Connected to debugger");

    tokio::spawn(async move {
        info!("[rx] Waiting for samples");
        rx.for_each(|message| async {
            let data = message.unwrap().into_data();

            let mut deserializer = serde_json::Deserializer::from_slice(&data);
            deserializer.disable_recursion_limit();

            let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
            let v = structs::WebsocketResponse::deserialize(deserializer).unwrap();

            if let structs::WebsocketResponseResult::Profile { profile } = v.result {
                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis();

                let filename = format!("./.memgraphs/{}.txt", since_the_epoch);
                let f = File::create(filename).expect("Unable to create file");
                let mut writer = BufWriter::new(f);

                process(&mut writer, profile.head, "".to_string());
            }
        })
        .await;
    });

    info!("Ensuring debugger is running");

    tx.send(Message::Text(
        json!({"id": 0, "method": "Runtime.runIfWaitingForDebugger"}).to_string(),
    ))
    .await?;

    let sleep_delay = time::Duration::from_millis(args.frequency);

    info!("Sleeping for {}ms", args.delay);
    thread::sleep(time::Duration::from_millis(args.delay));

    ctrlc::set_handler(move || {
        info!("Collecting files from temporary directory");
        let temp_dir = args.temp_dir.to_owned();
        let files: Vec<PathBuf> = fs::read_dir(&temp_dir)
            .expect("Unable to list files in temporary directory")
            .map(|p| p.unwrap().path())
            .collect();

        info!("Found {} files", files.len());

        if files.len() > 0 {
            let mut opt = inferno::flamegraph::Options::default();
            opt.colors =
                inferno::flamegraph::Palette::Multi(inferno::flamegraph::color::MultiPalette::Js);
            opt.bgcolors = Some(inferno::flamegraph::color::BackgroundColor::Grey);

            let f = File::create("memgraph.svg").expect("Unable to create file");
            let f = BufWriter::new(f);

            info!("Generating flamegraph");
            match inferno::flamegraph::from_files(&mut opt, &files, f) {
                Ok(()) => info!("Generated flamegraph"),
                Err(err) => error!("Unable to generate flamegraph: {}", err),
            };

            fs::remove_dir_all(temp_dir).expect("Could not delete files");
        }
        std::process::exit(0);
    })?;

    info!("Starting to sample, press Ctrl+C/Cmd+C to finish");
    tx.send(Message::Text(
        json!({"id": 0, "method": "HeapProfiler.startSampling"}).to_string(),
    ))
    .await?;

    loop {
        debug!("Sending command to get sample");
        tx.send(Message::Text(
            json!({"id": 1, "method": "HeapProfiler.getSamplingProfile"}).to_string(),
        ))
        .await?;
        thread::sleep(sleep_delay);
    }
}
