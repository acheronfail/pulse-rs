use std::fmt::Debug;
use std::sync::mpsc::{SendError, Sender};

use crate::api::PAEvent;

// TODO: `SendError` is a little too restrictive, see the CLI's subscribe implementation
pub trait EventSender: Debug + Send {
    fn send(&self, ev: PAEvent) -> Result<(), SendError<PAEvent>>;
}

impl EventSender for Sender<PAEvent> {
    fn send(&self, ev: PAEvent) -> Result<(), SendError<PAEvent>> {
        self.send(ev)
    }
}
