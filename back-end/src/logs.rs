use std::collections::VecDeque;
use std::sync::Arc;

use serde_json::json;
use socketioxide::extract::SocketRef;
use socketioxide::SocketIo;
use tokio::sync::Mutex;
use tracing::{event, Level};

pub(crate) struct LogsSocket {
    io: SocketIo,
}
impl LogsSocket {
    pub(crate) fn get_socket(&self) -> SocketIo {
        self.io.clone()
    }
}

impl Drop for LogsSocket {
    fn drop(&mut self) {
        let io = self.io.clone();
        tokio::task::spawn(async move {
            if let Err(err) = io.emit("goodbye", &json!({})).await {
                event!(Level::ERROR, ?err, "Failed to announce shutting down");
            }
        });
    }
}

pub async fn setup_socket(io: SocketIo, _messages: Arc<Mutex<VecDeque<String>>>) -> LogsSocket {
    io.ns("/", |_socket: SocketRef| async move {
        // let messages = messages.lock().await;

        // if let Err(err) = socket.emit("messages", json!({ "messages": &*messages })) {
        //     event!(
        //         Level::ERROR,
        //         ?err,
        //         "Failed to send messages to connected client"
        //     );
        // }
    });

    LogsSocket { io }
}
