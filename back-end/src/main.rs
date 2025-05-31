mod broadcast;
mod logs;
mod router;
mod routes;
mod server;
mod state;
mod states;
mod tasks;
mod utils;

use std::collections::VecDeque;
use std::env::VarError;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{env, thread};

use broadcast::setup_broadcast;
use color_eyre::config::HookBuilder;
use color_eyre::eyre;
use socketioxide::SocketIo;
use states::config::Config;
use tokio::signal;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{Level, event};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::logs::setup_socket;
use crate::router::build_router;
use crate::server::setup_server;
use crate::state::ApplicationState;
use crate::tasks::monitor_stdin;

#[expect(clippy::unnecessary_wraps)]
fn build_configs() -> Result<Config, eyre::Report> {
    let config = Config {};

    Ok(config)
}

/// starts all the tasks, such as the web server, the key refresh, ...
/// ensures all tasks are gracefully shutdown in case of error, ctrl+c or sigterm
async fn start_tasks() -> Result<(), eyre::Report> {
    let config = build_configs()?;

    // this channel is used to communicate between
    // tasks and this function, in the case that a task fails, they'll send a message on the shutdown channel
    // after which we'll gracefully terminate other services
    let token = CancellationToken::new();

    let application_state = ApplicationState::new(config);

    let (layer, io) = SocketIo::new_layer();

    let (sender, receiver) = tokio::sync::mpsc::channel::<String>(32);

    let tasks = TaskTracker::new();

    {
        let messages = Arc::new(Mutex::<VecDeque<String>>::new(VecDeque::new()));
        let websocket = { setup_socket(io, messages).await };

        let token = token.clone();

        tasks.spawn(async move {
            let _guard = token.clone().drop_guard();

            setup_broadcast(websocket, receiver, token).await;

            event!(Level::INFO, "Websocket broadcast shutdown");
        });
    }

    {
        let bind_to = SocketAddr::from(([0, 0, 0, 0], 3000));
        let router = build_router(application_state, layer);

        let token = token.clone();

        tasks.spawn(async move {
            let _guard = token.clone().drop_guard();

            match setup_server(bind_to, router, token).await {
                Err(err) => {
                    event!(Level::ERROR, ?err, "Webserver died");
                },
                Ok(()) => {
                    event!(Level::INFO, "Webserver shut down gracefully");
                },
            }
        });
    }

    {
        let token = token.clone();

        let _handle = thread::spawn(|| {
            monitor_stdin(sender, token);
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

    // close the tracker, otherwise wait doesn't work
    tasks.close();

    // wait for the task that holds the server to exit gracefully
    // it listens to shutdown_send
    if timeout(Duration::from_millis(10000), tasks.wait())
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

fn build_default_directive() -> EnvFilter {
    EnvFilter::builder()
        .parse(format!("INFO,{}=TRACE", env!("CARGO_CRATE_NAME")))
        .expect("Default filter should always work")
}

fn init_tracing() -> Result<(), eyre::Report> {
    let (filter, filter_parsing_error) = match env::var(EnvFilter::DEFAULT_ENV) {
        Ok(user_directive) => match EnvFilter::builder().parse(user_directive) {
            Ok(filter) => (filter, None),
            Err(error) => (build_default_directive(), Some(eyre::Report::new(error))),
        },
        Err(VarError::NotPresent) => (build_default_directive(), None),
        Err(error @ VarError::NotUnicode(_)) => {
            (build_default_directive(), Some(eyre::Report::new(error)))
        },
    };

    let registry = tracing_subscriber::registry();

    #[cfg(feature = "tokio-console")]
    let registry = registry.with(console_subscriber::ConsoleLayer::builder().spawn());

    registry
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(tracing_error::ErrorLayer::default())
        .try_init()?;

    filter_parsing_error.map_or(Ok(()), Err)
}

fn main() -> Result<(), eyre::Report> {
    HookBuilder::default()
        .capture_span_trace_by_default(true)
        .display_env_section(false)
        .install()?;

    init_tracing()?;

    // initialize the runtime
    let rt = tokio::runtime::Runtime::new().unwrap();

    // start service
    let result: Result<(), eyre::Report> = rt.block_on(start_tasks());

    result
}
