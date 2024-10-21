mod logs;
mod router;
mod routes;
mod server;
mod state;
mod states;
mod tasks;
mod utils;

use std::collections::VecDeque;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde_json::json;
use socketioxide::SocketIo;
use states::config::Config;
use tokio::signal;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{event, Level};
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::logs::setup_socket;
use crate::router::build_router;
use crate::server::setup_server;
use crate::state::ApplicationState;
use crate::tasks::monitor_stdin;

#[allow(clippy::unnecessary_wraps)]
fn build_configs() -> Result<Config, color_eyre::eyre::Report> {
    let config = Config {};

    Ok(config)
}

/// starts all the tasks, such as the web server, the key refresh, ...
/// ensures all tasks are gracefully shutdown in case of error, ctrl+c or sigterm
async fn start_tasks() -> Result<(), color_eyre::Report> {
    let config = build_configs()?;

    // this channel is used to communicate between
    // tasks and this function, in the case that a task fails, they'll send a message on the shutdown channel
    // after which we'll gracefully terminate other services
    let token = CancellationToken::new();

    let application_state = ApplicationState::new(config);

    let (layer, io) = SocketIo::new_layer();

    let router = build_router(application_state, layer);

    let bind_to = SocketAddr::from(([0, 0, 0, 0], 3000));

    let messages = Arc::new(Mutex::<VecDeque<String>>::new(VecDeque::new()));

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(32);

    let word_socket = { setup_socket(io, messages).await };

    let mut tasks = JoinSet::new();

    {
        let token = token.clone();

        tasks.spawn(async move {
            let _guard = token.clone().drop_guard();

            loop {
                let message = receiver.recv().await;

                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("Computer time wrong")
                    .as_millis();

                let json = json!({
                    "timestamp": timestamp,
                    "message": message
                });

                if let Err(err) = word_socket.get_socket().emit("input", &json) {
                    event!(Level::ERROR, ?err, "Failed to send line");
                }
            }
        });
    }

    {
        let token = token.clone();

        tasks.spawn(async move {
            let _guard = token.clone().drop_guard();

            match monitor_stdin(sender, token).await {
                Err(err) => {
                    event!(Level::ERROR, ?err, "Stdin died");
                },
                Ok(()) => {
                    event!(Level::INFO, "Stdin shut down");
                },
            }
        });
    }

    {
        let token = token.clone();

        tasks.spawn(async move {
            let _guard = token.clone().drop_guard();

            match setup_server(bind_to, router, token).await {
                Err(err) => {
                    event!(Level::ERROR, ?err, "Server died");
                },
                Ok(()) => {
                    event!(Level::INFO, "Server shut down");
                },
            }
        });
    }

    // now we wait forever for either
    // * sigterm
    // * ctrl + c
    // * a message on the shutdown channel, sent either by the server task or
    // another task when they complete (which means they failed)
    tokio::select! {
        _ = utils::wait_for_sigterm() => {
            event!(Level::WARN, "Sigterm detected, stopping all tasks");
        },
        _ = signal::ctrl_c() => {
            event!(Level::WARN, "CTRL+C detected, stopping all tasks");
        },
        () = token.cancelled() => {
            event!(Level::ERROR, "Underlying task stopped, stopping all others tasks");
        },
    };

    // announce cancel
    token.cancel();

    // wait for the task that holds the server to exit gracefully
    // it listens to shutdown_send
    if timeout(Duration::from_millis(10000), tasks.shutdown())
        .await
        .is_err()
    {
        event!(
            Level::ERROR,
            message = "Tasks didn't stop within allotted time!"
        );
    }

    event!(Level::INFO, "Goodbye");

    Ok(())
}

fn main() -> Result<(), color_eyre::Report> {
    color_eyre::config::HookBuilder::default()
        .capture_span_trace_by_default(false)
        .install()?;

    let rust_log_value = env::var(EnvFilter::DEFAULT_ENV)
        .unwrap_or_else(|_| format!("INFO,{}=TRACE", env!("CARGO_PKG_NAME").replace('-', "_")));

    // set up logger
    tracing_subscriber::registry()
        .with(EnvFilter::builder().parse(rust_log_value).unwrap())
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_error::ErrorLayer::default())
        .init();

    // initialize the runtime
    let rt = tokio::runtime::Runtime::new().unwrap();

    // start service
    let result: Result<(), color_eyre::Report> = rt.block_on(start_tasks());

    result
}
