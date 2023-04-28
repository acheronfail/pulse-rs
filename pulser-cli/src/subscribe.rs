use std::error::Error;
use std::fmt::Debug;
use std::io::ErrorKind;
use std::sync::Arc;

use mio::{Events, Interest, Poll, Token, Waker};
use mio_misc::channel::channel as mio_channel;
use mio_misc::queue::NotificationQueue;
use mio_misc::NotificationId;
use pulser::api::{PAEvent, PAMask};
use pulser::sender::EventSender;
use pulser::simple::PulseAudio;
use signal_hook::consts::signal::*;
use signal_hook_mio::v0_8::Signals;

use crate::json_print;

struct Sender(mio_misc::channel::Sender<PAEvent>);

impl Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sender(mio_misc::channel::Sender<PAEvent>)")
    }
}

impl EventSender for Sender {
    fn send(&self, ev: PAEvent) -> Result<(), std::sync::mpsc::SendError<PAEvent>> {
        match self.0.send(ev) {
            Ok(()) => Ok(()),
            Err(err) => match err {
                mio_misc::channel::SendError::Disconnected(ev) => {
                    Err(std::sync::mpsc::SendError(ev))
                }
                _ => unimplemented!(),
            },
        }
    }
}

pub fn subscribe(pa: PulseAudio, mask: PAMask) -> Result<(), Box<dyn Error>> {
    let mut poll = Poll::new()?;

    // ---

    let pa_event_token = Token(0);
    let pa_event_waker = Arc::new(Waker::new(poll.registry(), pa_event_token).unwrap());
    let pa_event_queue = Arc::new(NotificationQueue::new(pa_event_waker));
    let pa_event_notifier = Arc::clone(&pa_event_queue);
    let pa_event_notification_id = NotificationId::gen_next();
    let (tx, rx) = mio_channel(pa_event_notifier, pa_event_notification_id);

    pa.subscribe(mask, Box::new(Sender(tx)))?;

    // ---

    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    let signal_token = Token(1);
    poll.registry()
        .register(&mut signals, signal_token, Interest::READABLE)?;

    // ---

    let mut events = Events::with_capacity(10);
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
                Token(0) => {
                    // mio_extra's channel integration uses a queue to notify us of events - each notification
                    // corresponds to a sent event, so we must take care to drain those notifications
                    while let Some(_) = pa_event_queue.pop() {
                        let ev = rx.try_recv().unwrap();
                        json_print!(ev);
                    }
                }
                Token(1) => {
                    for signal in signals.pending() {
                        match signal {
                            SIGINT => break 'outer,
                            SIGTERM => break 'outer,
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
