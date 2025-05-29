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
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{env, thread};

use broadcast::setup_broadcast;
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

fn build_filter(with_tokio_runtime_trace: bool) -> EnvFilter {
    let filter_builder = EnvFilter::builder()
        .parse(
            env::var(EnvFilter::DEFAULT_ENV)
                .unwrap_or_else(|_| format!("INFO,{}=TRACE", env!("CARGO_CRATE_NAME"))),
        )
        .unwrap();

    if with_tokio_runtime_trace {
        filter_builder
            .add_directive("tokio=TRACE".parse().unwrap())
            .add_directive("runtime=TRACE".parse().unwrap())
    } else {
        filter_builder
    }
}

fn init_tracing() {
    #[cfg(feature = "console-subscriber")]
    let main_filter = build_filter(true);

    #[cfg(not(feature = "console-subscriber"))]
    let main_filter = build_filter(false);

    let registry = tracing_subscriber::registry().with(main_filter);

    // we'll need to do this hack until https://github.com/tokio-rs/tracing/issues/2929 is fixed
    #[cfg(feature = "console-subscriber")]
    let registry = registry.with(
        console_subscriber::ConsoleLayer::builder()
            .with_default_env()
            .spawn(),
    );

    registry
        .with(tracing_subscriber::fmt::layer().with_filter(build_filter(false)))
        .with(tracing_error::ErrorLayer::default())
        .init();
}

fn main() -> Result<(), color_eyre::Report> {
    color_eyre::config::HookBuilder::default()
        .capture_span_trace_by_default(false)
        .install()?;

    init_tracing();

    // initialize the runtime
    let rt = tokio::runtime::Runtime::new().unwrap();

    // start service
    let result: Result<(), color_eyre::Report> = rt.block_on(start_tasks());

    result
}
