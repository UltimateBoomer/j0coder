use tokio::sync::watch;

/// A cloneable, race-free cancellation signal for service and child tasks.
#[derive(Clone, Debug)]
pub struct Shutdown {
    receiver: watch::Receiver<bool>,
}

#[derive(Debug)]
pub struct ShutdownTrigger {
    sender: watch::Sender<bool>,
}

pub fn channel() -> (ShutdownTrigger, Shutdown) {
    let (sender, receiver) = watch::channel(false);
    (ShutdownTrigger { sender }, Shutdown { receiver })
}

impl ShutdownTrigger {
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }
}

impl Shutdown {
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.receiver.clone();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

/// Installs the process signal listener and returns its cancellation signal.
pub fn signal() -> Shutdown {
    let (trigger, shutdown) = channel();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        trigger.cancel();
    });
    shutdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_reaches_existing_and_future_waiters() {
        let (trigger, shutdown) = channel();
        let waiter = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.cancelled().await }
        });
        trigger.cancel();
        waiter.await.unwrap();
        shutdown.cancelled().await;
        assert!(shutdown.is_cancelled());
    }
}
