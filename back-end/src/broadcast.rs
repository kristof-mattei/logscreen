use std::time::SystemTime;

use serde_json::json;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;
use tracing::{Level, event};

use crate::logs::LogsSocket;

pub(crate) async fn setup_broadcast(
    websocket: LogsSocket,
    mut receiver: Receiver<String>,
    token: CancellationToken,
) {
    loop {
        #[expect(clippy::pattern_type_mismatch, reason = "Tokio macro")]
        let message = {
            tokio::select! {
                () = token.cancelled() => {
                    // The token was cancelled
                    break;
                },
                message = receiver.recv() => {
                    message
                }
            }
        };

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Computer time wrong")
            .as_millis();

        let json = json!({
            "timestamp": timestamp,
            "message": message
        });

        if let Err(err) = websocket.get_socket().emit("input", &json).await {
            event!(Level::ERROR, ?err, "Failed to send line");
        }
    }
}
