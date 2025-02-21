use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use tracing::{Level, event};

#[expect(clippy::needless_pass_by_value)]
pub(crate) fn monitor_stdin(sender: Sender<String>, token: CancellationToken) {
    let _guard = token.clone().drop_guard();

    let stdin = std::io::stdin();

    loop {
        let mut buffer = String::new();

        match stdin.read_line(&mut buffer) {
            Ok(_) => {
                if let Err(err) = sender.blocking_send(buffer.clone()) {
                    event!(
                        Level::ERROR,
                        ?err,
                        "Failed to send message to mpsc, stopping..."
                    );
                    break;
                }

                buffer.clear();
            },
            Err(err) => {
                event!(Level::ERROR, ?err, "Failure to read from stdin");
                break;
            },
        }
    }
}
