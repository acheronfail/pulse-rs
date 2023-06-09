use std::cell::RefCell;
use std::error::Error;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::thread;

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::channelmap::Position;
use libpulse_binding::context::introspect::{
    CardInfo,
    ClientInfo,
    ModuleInfo,
    SampleInfo,
    ServerInfo,
    SinkInfo,
    SinkInputInfo,
    SourceInfo,
    SourceOutputInfo,
};
use libpulse_binding::context::subscribe::Operation;
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::proplist::{properties, Proplist};
use libpulse_binding::volume::Volume;
use libpulse_sys::PA_INVALID_INDEX;

use super::api::*;
use super::util::updated_channel_volumes;
use crate::ignore::Ignore;
use crate::pulseaudio::api::VolumeReading;
use crate::sender::EventSender;

type Ctx = Rc<RefCell<Context>>;
type Res = Result<(), Box<dyn Error>>;

macro_rules! cb {
    ($f:expr, $ident:expr, $ctx:expr, $tx:expr) => {
        move |result| {
            match result {
                // The result we wanted, act on it
                ListResult::Item(inner) => {
                    if let Err(e) = (&mut $f)($ident.clone(), $ctx.clone(), inner) {
                        $tx.send(PAResponse::OpError(e.to_string())).ignore();
                    }
                }
                // An error occurred, check it and send an error event
                ListResult::Error => Self::handle_error(&$ctx, &$tx),
                // We reached the end of the list
                ListResult::End => {}
            }
        }
    };
}

macro_rules! impl_call_ident_both {
    ($ty:ident) => {
        paste::paste! {
            fn [<with_ $ty:snake>]<F>(&self, ident: PAIdent, mut f: F)
            where
                F: FnMut(PAIdent, Ctx, &$ty) -> Res + 'static,
            {
                let tx = self.tx.clone();
                let ctx = self.ctx.clone();
                let introspector = ctx.borrow_mut().introspect();
                match ident.clone() {
                    PAIdent::Index(idx) => introspector.[<get_ $ty:snake _by_index>](idx, cb!(f, ident, ctx, tx)),
                    PAIdent::Name(ref name) => introspector.[<get_ $ty:snake _by_name>](name, cb!(f, ident, ctx, tx)),
                };
            }
        }
    };
}

macro_rules! impl_call_ident_index {
    ($ty:ident) => {
        paste::paste! {
            fn [<with_ $ty:snake>]<F>(&self, idx: u32, mut f: F)
            where
                F: FnMut(PAIdent, Ctx, &$ty) -> Res + 'static,
            {
                let tx = self.tx.clone();
                let ctx = self.ctx.clone();
                let introspector = ctx.borrow_mut().introspect();
                let ident = PAIdent::Index(idx);
                introspector.[<get_ $ty:snake>](idx, cb!(f, ident, ctx, tx));
            }
        }
    };
}

macro_rules! impl_list_call {
    ($ty:ident) => {
        paste::paste! {
            fn [<get_ $ty:snake _list>](&self) {
                let introspector = self.ctx.borrow_mut().introspect();
                let tx = self.tx.clone();
                let ctx = self.ctx.clone();
                let mut v: Vec<[<PA $ty>]> = vec![];
                introspector.[<get_ $ty:snake _list>](move |result: ListResult<&$ty>| {
                    match result {
                        // Called for each item in the list
                        ListResult::Item(info) => v.push([<PA $ty>]::from(info)),
                        // Called at the end of the iteration, send the event back
                        ListResult::End => tx.send(PAResponse::[<$ty List>](v.clone())).ignore(),
                        // An error occurred, check it and send an error event
                        ListResult::Error => Self::handle_error(&ctx, &tx),
                    };
                });
            }
        }
    };
}

#[derive(Debug, Clone, Copy)]
pub enum StopReason {
    CommandSenderDropped,
    ExplicitDisconnect,
}

pub struct PulseAudioLoop {
    rx: Receiver<PACommand>,
    tx: Sender<PAResponse>,
    ctx: Rc<RefCell<Context>>,
    mainloop: Rc<RefCell<Mainloop>>,
}

impl PulseAudioLoop {
    impl_call_ident_both!(CardInfo);
    impl_call_ident_both!(SinkInfo);
    impl_call_ident_both!(SourceInfo);

    impl_call_ident_index!(ClientInfo);
    impl_call_ident_index!(ModuleInfo);
    impl_call_ident_index!(SinkInputInfo);
    impl_call_ident_index!(SourceOutputInfo);

    impl_list_call!(SinkInfo);
    impl_list_call!(SourceInfo);
    impl_list_call!(SinkInputInfo);
    impl_list_call!(SourceOutputInfo);
    impl_list_call!(ClientInfo);
    impl_list_call!(SampleInfo);
    impl_list_call!(CardInfo);
    impl_list_call!(ModuleInfo);

    // TODO: tokio support???
    /// Sets up a connection to PulseAudio. PulseAudio uses a loop-based asynchronous API, and so
    /// when this is called, a background thread will be created to setup up a threaded loop API for
    /// PulseAudio.
    ///
    /// If the `Receiver<PAResponse>` is dropped, then this will shut down PulseAudio's loop and clean
    /// up.
    pub fn start(
        app_name: impl AsRef<str> + Send + 'static,
    ) -> (Sender<PACommand>, Receiver<PAResponse>) {
        let (response_tx, response_rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel();

        // Run pulseaudio loop in background thread
        thread::spawn(move || {
            let pa = match PulseAudioLoop::init(app_name.as_ref(), response_tx.clone(), cmd_rx) {
                Ok(pa) => pa,
                Err(e) => panic!("An error occurred while connecting to pulseaudio: {}", e),
            };

            match pa.start_loop() {
                Ok(reason) => match reason {
                    StopReason::CommandSenderDropped | StopReason::ExplicitDisconnect => {}
                },
                Err(e) => panic!("An error occurred while interfacing with pulseaudio: {}", e),
            }

            // Signal that we're done
            response_tx.send(PAResponse::Disconnected).ignore();
        });

        (cmd_tx, response_rx)
    }

    // https://freedesktop.org/software/pulseaudio/doxygen/threaded_mainloop.html
    // https://gavv.net/articles/pulseaudio-under-the-hood/#asynchronous-api
    // https://docs.rs/libpulse-binding/2.26.0/libpulse_binding/mainloop/threaded/index.html#example
    fn init(
        with_app_name: impl AsRef<str>,
        tx: Sender<PAResponse>,
        rx: Receiver<PACommand>,
    ) -> Result<PulseAudioLoop, Box<dyn Error>> {
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
            let mainloop_ref = mainloop.clone();
            let context_ref = ctx.clone();
            ctx.borrow_mut().set_state_callback(Some(Box::new(move || {
                // TODO: investigate removing unsafe??
                // let state = context_ref.borrow_mut().get_state();
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

        // release lock to allow loop to continue
        mainloop.borrow_mut().unlock();

        Ok(PulseAudioLoop {
            tx,
            rx,
            ctx,
            mainloop,
        })
    }

    pub fn start_loop(&self) -> Result<StopReason, Box<dyn Error>> {
        loop {
            // wait for our next command
            let cmd = match self.rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => {
                    self.mainloop.borrow_mut().stop();
                    return Ok(StopReason::CommandSenderDropped);
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

            match cmd {
                PACommand::GetServerInfo => self.get_server_info(),
                PACommand::GetDefaultSink => self.get_default_sink(),
                PACommand::GetDefaultSource => self.get_default_source(),
                PACommand::SetDefaultSink(id) => self.set_default_sink(id),
                PACommand::SetDefaultSource(id) => self.set_default_source(id),

                PACommand::GetCardInfo(id) => self.get_card_info(id),
                PACommand::SetCardProfile(id, profile) => self.set_card_profile(id, &profile),
                PACommand::SetPortLatencyOffset(card, port, offset) => {
                    self.set_port_latency_offset(&card, &port, offset)
                }

                PACommand::GetClientInfo(idx) => self.get_client_info(idx),
                PACommand::KillClient(idx) => self.kill_client(idx),

                PACommand::GetModuleInfo(idx) => self.get_module_info(idx),
                PACommand::LoadModule(name, args) => self.load_module(&name, &args),
                PACommand::UnloadModule(idx) => self.unload_module(idx),

                PACommand::GetSinkInfo(id) => self.get_sink_info(id),
                PACommand::GetSinkMute(id) => self.get_sink_mute(id),
                PACommand::GetSinkVolume(id) => self.get_sink_volume(id),
                PACommand::SetSinkMute(id, mute) => self.set_sink_mute(id, mute),
                PACommand::SetSinkVolume(id, vol) => self.set_sink_volume(id, vol),
                PACommand::SetSinkPort(id, ref name) => self.set_sink_port(id, name),
                PACommand::SuspendSink(id, suspend) => self.suspend_sink(id, suspend),

                PACommand::GetSourceInfo(id) => self.get_source_info(id),
                PACommand::GetSourceMute(id) => self.get_source_mute(id),
                PACommand::GetSourceVolume(id) => self.get_source_volume(id),
                PACommand::SetSourceMute(id, mute) => self.set_source_mute(id, mute),
                PACommand::SetSourceVolume(id, vol) => self.set_source_volume(id, vol),
                PACommand::SetSourcePort(id, ref name) => self.set_source_port(id, name),
                PACommand::SuspendSource(id, suspend) => self.suspend_source(id, suspend),

                PACommand::GetSinkInputInfo(idx) => self.get_sink_input_info(idx),
                PACommand::GetSinkInputMute(idx) => self.get_sink_input_mute(idx),
                PACommand::GetSinkInputVolume(idx) => self.get_sink_input_volume(idx),
                PACommand::SetSinkInputMute(idx, mute) => self.set_sink_input_mute(idx, mute),
                PACommand::SetSinkInputVolume(idx, vol) => self.set_sink_input_volume(idx, vol),
                PACommand::MoveSinkInput(idx, sink_id) => self.move_sink_input(idx, sink_id),
                PACommand::KillSinkInput(idx) => self.kill_sink_input(idx),

                PACommand::GetSourceOutputInfo(idx) => self.get_source_output_info(idx),
                PACommand::GetSourceOutputMute(idx) => self.get_source_output_mute(idx),
                PACommand::GetSourceOutputVolume(idx) => self.get_source_output_volume(idx),
                PACommand::SetSourceOutputMute(idx, mute) => self.set_source_output_mute(idx, mute),
                PACommand::SetSourceOutputVolume(idx, vol) => {
                    self.set_source_output_volume(idx, vol)
                }
                PACommand::MoveSourceOutput(idx, source_id) => {
                    self.move_source_output(idx, source_id)
                }
                PACommand::KillSourceOutput(idx) => self.kill_source_output(idx),

                PACommand::GetCardInfoList => self.get_card_info_list(),
                PACommand::GetClientInfoList => self.get_client_info_list(),
                PACommand::GetModuleInfoList => self.get_module_info_list(),
                PACommand::GetSampleInfoList => self.get_sample_info_list(),
                PACommand::GetSinkInfoList => self.get_sink_info_list(),
                PACommand::GetSinkInputInfoList => self.get_sink_input_info_list(),
                PACommand::GetSourceInfoList => self.get_source_info_list(),
                PACommand::GetSourceOutputInfoList => self.get_source_output_info_list(),

                PACommand::Subscribe(mask, tx) => self.setup_subscribe(mask, tx),

                PACommand::Disconnect => {
                    self.mainloop.borrow_mut().unlock();
                    self.mainloop.borrow_mut().stop();
                    return Ok(StopReason::ExplicitDisconnect);
                }
            }

            // resume mainloop
            self.mainloop.borrow_mut().unlock();
        }
    }

    /*
     * Server
     */

    fn with_server_info<F>(&self, mut f: F)
    where
        F: FnMut(&ServerInfo) + 'static,
    {
        let introspector = self.ctx.borrow_mut().introspect();
        introspector.get_server_info(move |info| f(info));
    }

    fn get_server_info(&self) {
        let tx = self.tx.clone();
        self.with_server_info(move |info| {
            tx.send(PAResponse::ServerInfo(info.into())).ignore();
        });
    }

    fn get_default_sink(&self) {
        let tx = self.tx.clone();
        self.with_server_info(move |info| {
            tx.send(PAResponse::DefaultSink(
                info.default_sink_name
                    .as_ref()
                    .map(|n| PAIdent::Name(n.to_string())),
            ))
            .ignore();
        });
    }

    fn set_default_sink(&self, ident: PAIdent) {
        match ident {
            // if the identifier is a name, just set the default
            PAIdent::Name(name) => {
                self.ctx
                    .borrow_mut()
                    .set_default_sink(&name, Self::success_cb(self.ctx.clone(), self.tx.clone()));
            }
            // but if it's an index, first find the index, and then set the default
            PAIdent::Index(idx) => {
                let tx = self.tx.clone();
                self.with_sink_info(ident, move |_, ctx, info| {
                    info.name
                        .as_ref()
                        .map(|name| {
                            ctx.borrow_mut()
                                .set_default_sink(&name, Self::success_cb(ctx.clone(), tx.clone()));
                        })
                        .ok_or_else(|| format!("Failed to find sink with id: {}", idx).into())
                })
            }
        }
    }

    fn get_default_source(&self) {
        let tx = self.tx.clone();
        self.with_server_info(move |info| {
            tx.send(PAResponse::DefaultSource(
                info.default_source_name
                    .as_ref()
                    .map(|n| PAIdent::Name(n.to_string())),
            ))
            .ignore();
        });
    }

    fn set_default_source(&self, ident: PAIdent) {
        match ident {
            // if the identifier is a name, just set the default
            PAIdent::Name(name) => {
                self.ctx
                    .borrow_mut()
                    .set_default_source(&name, Self::success_cb(self.ctx.clone(), self.tx.clone()));
            }
            // but if it's an index, first find the index, and then set the default
            PAIdent::Index(idx) => {
                let tx = self.tx.clone();
                self.with_source_info(ident, move |_, ctx, info| {
                    info.name
                        .as_ref()
                        .map(|name| {
                            ctx.borrow_mut().set_default_source(
                                &name,
                                Self::success_cb(ctx.clone(), tx.clone()),
                            );
                        })
                        .ok_or_else(|| format!("Failed to find source with id: {}", idx).into())
                })
            }
        }
    }

    /*
     * Cards
     */

    fn get_card_info(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_card_info(ident, move |_, _, info| {
            tx.send(PAResponse::CardInfo(info.into()))?;
            Ok(())
        });
    }

    fn set_card_profile(&self, ident: PAIdent, profile: &String) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_card_profile_by_index(
                    idx,
                    profile,
                    Some(Self::success_cb(ctx, tx)),
                );
            }
            PAIdent::Name(ref name) => {
                introspector.set_card_profile_by_name(
                    name,
                    profile,
                    Some(Self::success_cb(ctx, tx)),
                );
            }
        }
    }

    fn set_port_latency_offset(&self, card: &String, port: &String, offset: i64) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        introspector.set_port_latency_offset(card, port, offset, Some(Self::success_cb(ctx, tx)));
    }

    /*
     * Clients
     */

    fn get_client_info(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_client_info(idx, move |_, _, info| {
            tx.send(PAResponse::ClientInfo(info.into())).ignore();
            Ok(())
        });
    }

    fn kill_client(&self, idx: u32) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        introspector.kill_client(idx, Self::success_cb(self.ctx.clone(), self.tx.clone()));
    }

    /*
     * Modules
     */

    fn get_module_info(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_module_info(idx, move |_, _, info| {
            tx.send(PAResponse::ModuleInfo(info.into())).ignore();
            Ok(())
        });
    }

    fn load_module(&self, name: &String, argument: &String) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let mut introspector = self.ctx.borrow_mut().introspect();
        introspector.load_module(name, argument, move |index: u32| {
            if index == PA_INVALID_INDEX {
                Self::handle_error(&ctx, &tx);
            } else {
                tx.send(PAResponse::ModuleLoaded(index)).ignore();
            }
        });
    }

    fn unload_module(&self, idx: u32) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        introspector.unload_module(idx, Self::success_cb(self.ctx.clone(), self.tx.clone()));
    }

    /*
     * Subscriptions
     */

    fn setup_subscribe(&self, mask: PAMask, tx: Box<dyn EventSender>) {
        self.ctx
            .borrow_mut()
            .subscribe(mask, Self::success_cb(self.ctx.clone(), self.tx.clone()));

        let ctx = self.ctx.clone();
        self.ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
            move |facility, operation, index| {
                // SAFETY: as per libpulse_binding's documentation, this should be safe
                let operation = operation.unwrap();
                let kind = facility.unwrap();

                // send off a subscription event
                let kind = PAFacility(kind);
                let id = PAIdent::Index(index);
                let res = match operation {
                    Operation::New => tx.send(PAEvent::SubscriptionNew(kind, id)),
                    Operation::Removed => tx.send(PAEvent::SubscriptionRemoved(kind, id)),
                    Operation::Changed => tx.send(PAEvent::SubscriptionChanged(kind, id)),
                };

                // No one is listening to these events anymore, so remove the subscribe callback
                if let Err(SendError(_)) = res {
                    // TODO: verify with pa docs if this is enough, or if we need to set the mask to 0
                    ctx.borrow_mut().set_subscribe_callback(None);
                }
            },
        )));
    }

    /*
     * Sinks
     */

    fn get_sink_info(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_sink_info(ident, move |_, _, info| {
            tx.send(PAResponse::SinkInfo(info.into())).ignore();
            Ok(())
        });
    }

    fn get_sink_mute(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_sink_info(ident, move |ident, _, info| {
            tx.send(PAResponse::Mute(ident, info.mute)).ignore();
            Ok(())
        });
    }

    fn set_sink_mute(&self, ident: PAIdent, mute: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_sink_mute_by_index(idx, mute, Some(Self::success_cb(ctx, tx)))
            }
            PAIdent::Name(ref name) => {
                introspector.set_sink_mute_by_name(name, mute, Some(Self::success_cb(ctx, tx)))
            }
        };
    }

    fn get_sink_volume(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_sink_info(ident, move |ident, _, info| {
            tx.send(PAResponse::Volume(
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
        self.with_sink_info(ident, move |ident, ctx, info| {
            let mut introspector = ctx.borrow_mut().introspect();
            let cv = updated_channel_volumes(info.volume, &volume_spec);
            let tx = tx.clone();
            let ctx = ctx.clone();
            match ident {
                PAIdent::Index(idx) => {
                    introspector.set_sink_volume_by_index(idx, &cv, Some(Self::success_cb(ctx, tx)))
                }
                PAIdent::Name(ref name) => {
                    introspector.set_sink_volume_by_name(name, &cv, Some(Self::success_cb(ctx, tx)))
                }
            };

            Ok(())
        });
    }

    fn set_sink_port(&self, ident: PAIdent, port: &String) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_sink_port_by_index(idx, port, Some(Self::success_cb(ctx, tx)));
            }
            PAIdent::Name(ref name) => {
                introspector.set_sink_port_by_name(name, port, Some(Self::success_cb(ctx, tx)));
            }
        }
    }

    fn suspend_sink(&self, ident: PAIdent, suspend: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.suspend_sink_by_index(idx, suspend, Some(Self::success_cb(ctx, tx)));
            }
            PAIdent::Name(ref name) => {
                introspector.suspend_sink_by_name(name, suspend, Some(Self::success_cb(ctx, tx)));
            }
        }
    }

    /*
     * Sources
     */

    fn get_source_info(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_source_info(ident, move |_, _, info| {
            tx.send(PAResponse::SourceInfo(info.into())).ignore();
            Ok(())
        });
    }

    fn get_source_mute(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_source_info(ident, move |ident, _, info| {
            tx.send(PAResponse::Mute(ident, info.mute)).ignore();
            Ok(())
        });
    }

    fn set_source_mute(&self, ident: PAIdent, mute: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_source_mute_by_index(idx, mute, Some(Self::success_cb(ctx, tx)))
            }
            PAIdent::Name(ref name) => {
                introspector.set_source_mute_by_name(name, mute, Some(Self::success_cb(ctx, tx)))
            }
        };
    }

    fn get_source_volume(&self, ident: PAIdent) {
        let tx = self.tx.clone();
        self.with_source_info(ident, move |ident, _, info| {
            tx.send(PAResponse::Volume(
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
        self.with_source_info(ident, move |ident, ctx, info| {
            let mut introspector = ctx.borrow_mut().introspect();
            let cv = updated_channel_volumes(info.volume, &volume_spec);
            let tx = tx.clone();
            let ctx = ctx.clone();
            match ident {
                PAIdent::Index(idx) => introspector.set_source_volume_by_index(
                    idx,
                    &cv,
                    Some(Self::success_cb(ctx, tx)),
                ),
                PAIdent::Name(ref name) => introspector.set_source_volume_by_name(
                    name,
                    &cv,
                    Some(Self::success_cb(ctx, tx)),
                ),
            };

            Ok(())
        });
    }

    fn set_source_port(&self, ident: PAIdent, port: &String) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.set_source_port_by_index(idx, port, Some(Self::success_cb(ctx, tx)));
            }
            PAIdent::Name(ref name) => {
                introspector.set_source_port_by_name(name, port, Some(Self::success_cb(ctx, tx)));
            }
        }
    }

    fn suspend_source(&self, ident: PAIdent, suspend: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match ident {
            PAIdent::Index(idx) => {
                introspector.suspend_source_by_index(idx, suspend, Some(Self::success_cb(ctx, tx)));
            }
            PAIdent::Name(ref name) => {
                introspector.suspend_source_by_name(name, suspend, Some(Self::success_cb(ctx, tx)));
            }
        }
    }

    /*
     * Sink Inputs
     */

    fn get_sink_input_info(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_sink_input_info(idx, move |_, _, info| {
            tx.send(PAResponse::SinkInputInfo(info.into())).ignore();
            Ok(())
        });
    }

    fn get_sink_input_mute(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_sink_input_info(idx, move |ident, _, info| {
            tx.send(PAResponse::Mute(ident, info.mute)).ignore();
            Ok(())
        });
    }

    fn get_sink_input_volume(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_sink_input_info(idx, move |ident, _, info| {
            tx.send(PAResponse::Volume(
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

    fn set_sink_input_mute(&self, idx: u32, mute: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        introspector.set_sink_input_mute(idx, mute, Some(Self::success_cb(ctx, tx)));
    }

    fn set_sink_input_volume(&self, idx: u32, volume_spec: VolumeSpec) {
        let tx = self.tx.clone();
        self.with_sink_input_info(idx, move |_, ctx, info| {
            let mut introspector = ctx.borrow_mut().introspect();
            let cv = updated_channel_volumes(info.volume, &volume_spec);
            let tx = tx.clone();
            let ctx = ctx.clone();
            introspector.set_sink_input_volume(idx, &cv, Some(Self::success_cb(ctx, tx)));

            Ok(())
        });
    }

    fn move_sink_input(&self, idx: u32, sink: PAIdent) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match sink {
            PAIdent::Index(sink_idx) => introspector.move_sink_input_by_index(
                idx,
                sink_idx,
                Some(Self::success_cb(ctx, tx)),
            ),
            PAIdent::Name(ref name) => {
                introspector.move_sink_input_by_name(idx, name, Some(Self::success_cb(ctx, tx)))
            }
        };
    }

    fn kill_sink_input(&self, idx: u32) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        introspector.kill_sink_input(idx, Self::success_cb(self.ctx.clone(), self.tx.clone()));
    }

    /*
     * Source Outputs
     */

    fn get_source_output_info(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_source_output_info(idx, move |_, _, info| {
            tx.send(PAResponse::SourceOutputInfo(info.into())).ignore();
            Ok(())
        });
    }

    fn get_source_output_mute(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_source_output_info(idx, move |ident, _, info| {
            tx.send(PAResponse::Mute(ident, info.mute)).ignore();
            Ok(())
        });
    }

    fn get_source_output_volume(&self, idx: u32) {
        let tx = self.tx.clone();
        self.with_source_output_info(idx, move |ident, _, info| {
            tx.send(PAResponse::Volume(
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

    fn set_source_output_mute(&self, idx: u32, mute: bool) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        introspector.set_source_output_mute(idx, mute, Some(Self::success_cb(ctx, tx)));
    }

    fn set_source_output_volume(&self, idx: u32, volume_spec: VolumeSpec) {
        let tx = self.tx.clone();
        self.with_source_output_info(idx, move |_, ctx, info| {
            let mut introspector = ctx.borrow_mut().introspect();
            let cv = updated_channel_volumes(info.volume, &volume_spec);
            let tx = tx.clone();
            let ctx = ctx.clone();
            introspector.set_source_output_volume(idx, &cv, Some(Self::success_cb(ctx, tx)));

            Ok(())
        });
    }

    fn move_source_output(&self, idx: u32, source: PAIdent) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        match source {
            PAIdent::Index(source_idx) => introspector.move_source_output_by_index(
                idx,
                source_idx,
                Some(Self::success_cb(ctx, tx)),
            ),
            PAIdent::Name(ref name) => {
                introspector.move_source_output_by_name(idx, name, Some(Self::success_cb(ctx, tx)))
            }
        };
    }

    fn kill_source_output(&self, idx: u32) {
        let mut introspector = self.ctx.borrow_mut().introspect();
        introspector.kill_source_output(idx, Self::success_cb(self.ctx.clone(), self.tx.clone()));
    }

    /*
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

    fn success_cb(ctx: Ctx, tx: Sender<PAResponse>) -> Box<impl FnMut(bool)> {
        Box::new(move |success: bool| {
            if !success {
                Self::handle_error(&ctx, &tx)
            } else {
                tx.send(PAResponse::OpComplete).ignore();
            }
        })
    }

    fn handle_error(ctx: &Ctx, tx: &Sender<PAResponse>) {
        let err = ctx.borrow_mut().errno().to_string();
        tx.send(PAResponse::OpError(format!(
            "Operation failed: {}",
            err.unwrap_or("An unknown error occurred".into())
        )))
        .ignore();
    }
}
