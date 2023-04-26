use std::cell::RefCell;
use std::error::Error;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::channelmap::Position;
use libpulse_binding::context::introspect::{SinkInfo, SourceInfo};
use libpulse_binding::context::subscribe::{InterestMaskSet, Operation};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::proplist::{properties, Proplist};
use libpulse_binding::volume::Volume;

use super::api::*;
use super::util::updated_channel_volumes;
use crate::ignore::Ignore;
use crate::pulseaudio::api::VolumeReading;

type Ctx = Rc<RefCell<Context>>;
type Res = Result<(), Box<dyn Error>>;

macro_rules! impl_call {
    ($method:ident, $ty:ident) => {
        fn $method<F>(&self, ident: PAIdent, mut f: F)
        where
            F: FnMut(PAIdent, Ctx, &$ty) -> Res + 'static,
        {
            let tx = self.tx.clone();
            let ctx = self.ctx.clone();
            macro_rules! inner {
                ($prefix:ident, $cb:expr) => {
                    paste::paste! {
                        let introspector = ctx.borrow_mut().introspect();
                        match ident.clone() {
                            PAIdent::Index(idx) => introspector.[<$prefix index>](idx, $cb),
                            PAIdent::Name(ref name) => introspector.[<$prefix name>](name, $cb),
                        };
                    }
                };
            }

            paste::paste! {
                inner!([<$method _by_>], move |result| {
                    match result {
                        // The result we wanted, act on it
                        ListResult::Item(inner) => if let Err(e) = (&mut f)(ident.clone(), ctx.clone(), inner) {
                            tx.send(PAEvent::Error(e.to_string())).ignore();
                        },
                        // An error occurred, check it and send an error event
                        ListResult::Error => Self::handle_list_result_err(&ctx, &tx),
                        // We reached the end of the list
                        ListResult::End => {},
                    }
                });
            }
        }
    };
}

macro_rules! impl_list_call {
    ($pa_method:ident, $ty:ident) => {
        fn $pa_method(&self) {
            paste::paste! {
                let introspector = self.ctx.borrow_mut().introspect();
                let tx = self.tx.clone();
                let ctx = self.ctx.clone();
                let mut v: Vec<[<PA $ty>]> = vec![];
                introspector.$pa_method(move |result: ListResult<&$ty>| {
                    match result {
                        // Called for each item in the list
                        ListResult::Item(info) => v.push([<PA $ty>]::from(info)),
                        // Called at the end of the iteration, send the event back
                        ListResult::End => tx.send(PAEvent::[<$ty List>](v.clone())).ignore(),
                        // An error occurred, check it and send an error event
                        ListResult::Error => Self::handle_list_result_err(&ctx, &tx),
                    };
                });
            }
        }
    };
}

pub struct PulseAudio {
    rx: Receiver<PACommand>,
    tx: Sender<PAEvent>,
    ctx: Rc<RefCell<Context>>,
    mainloop: Rc<RefCell<Mainloop>>,
}

impl PulseAudio {
    // TODO: tokio support???
    /// Sets up a connection to PulseAudio. PulseAudio uses a loop-based asynchronous API, and so
    /// when this is called, a background thread will be created to setup up a threaded loop API for
    /// PulseAudio.
    ///
    /// If the `Receiver<PAEvent>` is dropped, then this will shut down PulseAudio's loop and clean
    /// up.
    pub fn connect(
        app_name: impl AsRef<str> + Send + 'static,
    ) -> (Sender<PACommand>, Receiver<PAEvent>) {
        let (result_tx, result_rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel();

        // Run pulseaudio loop in background thread
        thread::spawn(move || {
            let pa = match PulseAudio::init(app_name.as_ref(), result_tx, cmd_rx) {
                Ok(pa) => pa,
                Err(e) => panic!("An error occurred while connecting to pulseaudio: {}", e),
            };

            if let Err(e) = pa.start_loop() {
                panic!("An error occurred while interfacing with pulseaudio: {}", e);
            }
        });

        (cmd_tx, result_rx)
    }

    // https://freedesktop.org/software/pulseaudio/doxygen/threaded_mainloop.html
    // https://gavv.net/articles/pulseaudio-under-the-hood/#asynchronous-api
    // https://docs.rs/libpulse-binding/2.26.0/libpulse_binding/mainloop/threaded/index.html#example
    fn init(
        with_app_name: impl AsRef<str>,
        tx: Sender<PAEvent>,
        rx: Receiver<PACommand>,
    ) -> Result<PulseAudio, Box<dyn Error>> {
        let app_name = with_app_name.as_ref();

        let mut proplist = Proplist::new().ok_or("Failed to create PulseAudio Proplist")?;
        proplist
            .set_str(properties::APPLICATION_NAME, app_name)
            .map_err(|_| "Failed to update property list")?;

        let mainloop: Rc<RefCell<Mainloop>> = Rc::new(RefCell::new(
            Mainloop::new().ok_or("Failed to create PulseAudio Mainloop")?,
        ));
        let ctx = Rc::new(RefCell::new(
            Context::new_with_proplist(
                mainloop.borrow_mut().deref(),
                &format!("{}Context", app_name),
                &proplist,
            )
            .ok_or("Failed to create PulseAudio Context")?,
        ));

        // setup context
        {
            let mainloop_ref = Rc::clone(&mainloop);
            let context_ref = Rc::clone(&ctx);
            ctx.borrow_mut().set_state_callback(Some(Box::new(move || {
                let state = unsafe { (*context_ref.as_ptr()).get_state() };
                if matches!(state, State::Ready | State::Failed | State::Terminated) {
                    unsafe { (*mainloop_ref.as_ptr()).signal(false) };
                }
            })));
        }

        // connect to pulse
        ctx.borrow_mut().connect(None, FlagSet::NOFLAGS, None)?;

        // start mainloop
        mainloop.borrow_mut().lock();
        mainloop.borrow_mut().start()?;

        // loop, waiting for context to be ready
        loop {
            match ctx.borrow_mut().get_state() {
                State::Ready => break,
                State::Failed | State::Terminated => {
                    mainloop.borrow_mut().unlock();
                    mainloop.borrow_mut().stop();
                    return Err("Failed to connect".into());
                }
                _ => mainloop.borrow_mut().wait(),
            }
        }

        // context is ready now, so remove set state callback
        ctx.borrow_mut().set_state_callback(None);

        // setup subscribe mask and callbacks
        {
            let mask = InterestMaskSet::SINK | InterestMaskSet::SOURCE;
            ctx.borrow_mut().subscribe(mask, |success| {
                assert!(success, "subscription failed");
            });

            let tx = tx.clone();
            ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
                move |facility, operation, index| {
                    // SAFETY: as per libpulse_binding's documentation, this should be safe
                    let operation = operation.unwrap();
                    let kind = facility.unwrap();

                    // send off a subscription event
                    let id = PAIdent::Index(index);
                    match operation {
                        Operation::New => tx.send(PAEvent::SubscriptionNew(kind, id)).ignore(),
                        Operation::Removed => {
                            tx.send(PAEvent::SubscriptionRemoved(kind, id)).ignore()
                        }
                        Operation::Changed => {
                            tx.send(PAEvent::SubscriptionChanged(kind, id)).ignore()
                        }
                    }
                },
            )));
        }

        mainloop.borrow_mut().unlock();

        Ok(PulseAudio {
            tx,
            rx,
            ctx,
            mainloop,
        })
    }

    pub fn start_loop(&self) -> Result<!, Box<dyn Error>> {
        loop {
            // wait for our next command
            let cmd = match self.rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => {
                    self.mainloop.borrow_mut().unlock();
                    self.mainloop.borrow_mut().stop();
                    return Err("Command sender was closed, shutting down".into());
                }
            };

            // lock and pause mainloop
            self.mainloop.borrow_mut().lock();

            // verify connection state
            match self.ctx.borrow_mut().get_state() {
                State::Ready => {}
                _ => {
                    self.mainloop.borrow_mut().unlock();
                    return Err("Disconnected while working, shutting down".into());
                }
            }

            self.handle_cmd(cmd);

            // resume mainloop
            self.mainloop.borrow_mut().unlock();
        }
    }

    fn handle_cmd(&self, cmd: PACommand) {
        match cmd {
            PACommand::GetServerInfo => self.get_server_info(),

            PACommand::GetSinkInfoList => self.get_sink_info_list(),
            PACommand::GetSinkMute(id) => self.get_sink_mute(id),
            PACommand::GetSinkVolume(id) => self.get_sink_volume(id),
            PACommand::SetSinkMute(id, mute) => self.set_sink_mute(id, mute),
            PACommand::SetSinkVolume(id, vol) => self.set_sink_volume(id, vol),

            PACommand::GetSourceInfoList => self.get_source_info_list(),
            PACommand::GetSourceMute(id) => self.get_source_mute(id),
            PACommand::GetSourceVolume(id) => self.get_source_volume(id),
            PACommand::SetSourceMute(id, mute) => self.set_source_mute(id, mute),
            PACommand::SetSourceVolume(id, vol) => self.set_source_volume(id, vol),
        }
    }

    fn get_server_info(&self) {
        let tx = self.tx.clone();
        let introspector = self.ctx.borrow_mut().introspect();
        introspector.get_server_info(move |info| {
            tx.send(PAEvent::ServerInfo(info.into())).ignore();
        });
    }

    impl_call!(get_sink_info, SinkInfo);
    impl_call!(get_source_info, SourceInfo);

    impl_list_call!(get_sink_info_list, SinkInfo);
    impl_list_call!(get_source_info_list, SourceInfo);

    /*
     * Sinks
     */

    fn get_sink_mute(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.get_sink_info(ident, move |ident, _, info| {
            tx.send(PAEvent::Mute(ident, info.mute)).ignore();
            Ok(())
        });
    }

    fn set_sink_mute(&self, ident: PAIdent, mute: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_sink_mute_by_index(idx, mute, Some(Self::success_cb(tx)))
            }
            PAIdent::Name(ref name) => {
                introspector.set_sink_mute_by_name(name, mute, Some(Self::success_cb(tx)))
            }
        };
    }

    fn get_sink_volume(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.get_sink_info(ident, move |ident, _, info| {
            tx.send(PAEvent::Volume(
                ident,
                Self::read_volumes(
                    info.channel_map.get().into_iter(),
                    info.volume.get().into_iter(),
                ),
            ))
            .ignore();
            Ok(())
        });
    }

    fn set_sink_volume(&self, ident: PAIdent, volume_spec: VolumeSpec) {
        let tx = self.tx.clone();
        self.get_sink_info(ident, move |ident, ctx, info| {
            let mut introspector = ctx.borrow_mut().introspect();
            let cv = updated_channel_volumes(info.volume, &volume_spec);
            let tx = tx.clone();
            match ident {
                PAIdent::Index(idx) => {
                    introspector.set_sink_volume_by_index(idx, &cv, Some(Self::success_cb(tx)))
                }
                PAIdent::Name(ref name) => {
                    introspector.set_sink_volume_by_name(name, &cv, Some(Self::success_cb(tx)))
                }
            };

            Ok(())
        });
    }

    /*
     * Sources
     */

    fn get_source_mute(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.get_source_info(ident, move |ident, _, info| {
            tx.send(PAEvent::Mute(ident, info.mute)).ignore();
            Ok(())
        });
    }

    fn set_source_mute(&self, ident: PAIdent, mute: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_source_mute_by_index(idx, mute, Some(Self::success_cb(tx)))
            }
            PAIdent::Name(ref name) => {
                introspector.set_source_mute_by_name(name, mute, Some(Self::success_cb(tx)))
            }
        };
    }

    fn get_source_volume(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.get_source_info(ident, move |ident, _, info| {
            tx.send(PAEvent::Volume(
                ident,
                Self::read_volumes(
                    info.channel_map.get().into_iter(),
                    info.volume.get().into_iter(),
                )
                .into(),
            ))
            .ignore();
            Ok(())
        });
    }

    fn set_source_volume(&self, ident: PAIdent, volume_spec: VolumeSpec) {
        let tx = self.tx.clone();
        self.get_source_info(ident, move |ident, ctx, info| {
            let mut introspector = ctx.borrow_mut().introspect();
            let cv = updated_channel_volumes(info.volume, &volume_spec);
            let tx = tx.clone();
            match ident {
                PAIdent::Index(idx) => {
                    introspector.set_source_volume_by_index(idx, &cv, Some(Self::success_cb(tx)))
                }
                PAIdent::Name(ref name) => {
                    introspector.set_source_volume_by_name(name, &cv, Some(Self::success_cb(tx)))
                }
            };

            Ok(())
        });
    }

    /**
     * Util
     */

    fn read_volumes<'a>(
        channels: impl Iterator<Item = &'a Position>,
        volumes: impl Iterator<Item = &'a Volume>,
    ) -> VolumeReadings {
        channels
            .zip(volumes)
            .map(|(chan, vol)| VolumeReading::new(chan, vol))
            .collect()
    }

    fn success_cb(tx: Sender<PAEvent>) -> Box<impl FnMut(bool)> {
        Box::new(move |success| {
            tx.send(PAEvent::Complete(success)).ignore();
        })
    }

    fn handle_list_result_err(ctx: &Ctx, tx: &Sender<PAEvent>) {
        let err = ctx.borrow_mut().errno().to_string();
        let method = stringify!($method);
        tx.send(PAEvent::Error(format!(
            "{} failed: {}",
            method,
            err.unwrap_or("An unknown error occurred".into())
        )))
        .ignore();
    }
}
