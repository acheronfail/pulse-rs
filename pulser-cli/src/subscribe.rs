use std::error::Error;
use std::fmt::Debug;
use std::io::ErrorKind;
use std::sync::Arc;

use mio::{Events, Interest, Poll, Token, Waker};
use mio_misc::queue::NotificationQueue;
use mio_misc::NotificationId;
use pulser::api::{PAEvent, PAMask};
use pulser::sender::EventSender;
use pulser::simple::PulseAudio;
use signal_hook::consts::signal::*;
use signal_hook_mio::v0_8::Signals;

use crate::json_print;

// wrap up `mio_misc`'s sender so we can `impl EventSender` for it
struct Sender(mio_misc::channel::Sender<PAEvent>);

impl Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sender(mio_misc::channel::Sender<PAEvent>)")
    }
}

impl EventSender for Sender {
    fn send(&self, ev: PAEvent) -> Result<(), std::sync::mpsc::SendError<PAEvent>> {
        self.0.send(ev).map_err(|e| match e {
            // map to type expected from `EventSender` trait
            mio_misc::channel::SendError::Disconnected(ev) => std::sync::mpsc::SendError(ev),
            // we use `NotificationQueue` which is an unbounded queue
            mio_misc::channel::SendError::NotificationQueueFull => unreachable!(),
            // this should only occur if there's an IO error in `mio`'s `Waker`
            mio_misc::channel::SendError::Io(e) => {
                panic!("An underlying error occurred: {}", e)
            }
        })
    }
}

macro_rules! token {
    (PA_EVENT) => {
        Token(0)
    };
    (SIGNALS) => {
        Token(1)
    };
}

pub fn subscribe(pa: PulseAudio, mask: PAMask) -> Result<(), Box<dyn Error>> {
    let mut poll = Poll::new()?;

    // setup a channel that will land notifications in a wake-able queue each time a message is sent
    // then use this channel for subscribing to PulseAudio events
    let (pa_event_queue, rx) = {
        let waker = Arc::new(Waker::new(poll.registry(), token!(PA_EVENT)).unwrap());
        let queue = Arc::new(NotificationQueue::new(waker));
        let (tx, rx) = mio_misc::channel::channel(queue.clone(), NotificationId::gen_next());
        pa.subscribe(mask, Box::new(Sender(tx)))?;

        (queue, rx)
    };

    // register to receive wakeups for received signals
    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    poll.registry()
        .register(&mut signals, token!(SIGNALS), Interest::READABLE)?;

    // setup and start our event loop
    let mut events = Events::with_capacity(128);
    'outer: loop {
        match poll.poll(&mut events, None) {
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                // We get interrupt when a signal happens inside poll. That's non-fatal, just retry.
                events.clear();
                Ok(())
            }
            result => result,
        }?;

        for event in events.iter() {
            match event.token() {
                token!(PA_EVENT) => {
                    // mio_extra's channel integration uses a queue to notify us of events - each notification
                    // corresponds to a sent event, so we must take care to drain those notifications
                    while let Some(_) = pa_event_queue.pop() {
                        let ev = rx
                            .try_recv()
                            .expect("Channel notification count != channel item count");

                        json_print!(ev);
                    }
                }
                token!(SIGNALS) => {
                    for signal in signals.pending() {
                        match signal {
                            SIGINT | SIGTERM => break 'outer,
                            n => unreachable!("Received unexpected signal event in loop: {}", n),
                        }
                    }
                }
                token => unreachable!("Unknown token with id: {}", token.0),
            }
        }
    }

    Ok(())
}
