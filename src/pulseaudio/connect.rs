use std::cell::RefCell;
use std::error::Error;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::introspect::SinkInfo;
use libpulse_binding::context::subscribe::{InterestMaskSet, Operation};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::proplist::{properties, Proplist};

use super::api::{
    FacilityIdentifier, Ident, PulseAudioCommand, PulseAudioCommandResult, PulseAudioServerInfo
};

macro_rules! cb {
    ($ident:expr, $introspector:expr, $method:ident, $cb:expr) => {
        paste::paste! {
            match $ident {
                Ident::Index(ref idx) => $introspector.[<$method index>](*idx, $cb),
                Ident::Name(ref name) => $introspector.[<$method name>](name, $cb),
            };
        }
    };
}

pub struct PulseAudio;

impl PulseAudio {
    // https://freedesktop.org/software/pulseaudio/doxygen/threaded_mainloop.html
    // https://gavv.net/articles/pulseaudio-under-the-hood/#asynchronous-api
    // https://docs.rs/libpulse-binding/2.26.0/libpulse_binding/mainloop/threaded/index.html#example
    pub fn connect(
        with_app_name: impl AsRef<str>,
        cmd_rx: Receiver<PulseAudioCommand>,
        result_tx: Sender<PulseAudioCommandResult>,
    ) -> Result<!, Box<dyn Error>> {
        let app_name = with_app_name.as_ref();

        let mut proplist = Proplist::new().unwrap();
        proplist
            .set_str(properties::APPLICATION_NAME, app_name)
            .map_err(|_| "Failed to update property list")?;

        let mainloop: Rc<RefCell<Mainloop>> = Rc::new(RefCell::new(Mainloop::new().unwrap()));
        let ctx = Rc::new(RefCell::new(
            Context::new_with_proplist(
                mainloop.borrow_mut().deref(),
                &format!("{}Context", app_name),
                &proplist,
            )
            .unwrap(),
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
                    panic!();
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

            ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
                move |facility, operation, index| {
                    // SAFETY: as per libpulse_binding's documentation, this should be safe
                    let facility = facility.unwrap();
                    let operation = operation.unwrap();

                    let ident = FacilityIdentifier::new(facility, index);
                    match operation {
                        Operation::New => {}
                        Operation::Removed => {}
                        Operation::Changed => {
                            // TODO: send event out with `ident`
                            dbg!(ident);
                        }
                    }
                },
            )));
        }

        // opportunity to get initial state before starting send/recv loop
        {
            Self::get_server_info(&ctx, result_tx.clone());
        }

        mainloop.borrow_mut().unlock();

        // start mainloop
        loop {
            let cmd = cmd_rx.recv()?;

            // lock and pause mainloop
            mainloop.borrow_mut().lock();

            // verify connection state
            match ctx.borrow_mut().get_state() {
                State::Ready => {}
                _ => {
                    mainloop.borrow_mut().unlock();
                    todo!("disconnected while working");
                }
            }

            Self::handle_cmd(cmd, &ctx, &result_tx);

            // resume mainloop
            mainloop.borrow_mut().unlock();
        }
    }

    fn handle_cmd(
        cmd: PulseAudioCommand,
        ctx: &Rc<RefCell<Context>>,
        result_tx: &Sender<PulseAudioCommandResult>,
    ) {
        match cmd {
            PulseAudioCommand::GetServerInfo => Self::get_server_info(ctx, result_tx.clone()),
            PulseAudioCommand::GetMute(ident) => Self::get_mute(ident, ctx, result_tx.clone()),
            _ => todo!(),
        }
    }

    fn get_server_info(ctx: &Rc<RefCell<Context>>, tx: Sender<PulseAudioCommandResult>) {
        let introspector = ctx.borrow_mut().introspect();
        introspector.get_server_info(move |info| {
            tx.send(PulseAudioCommandResult::ServerInfo(PulseAudioServerInfo {
                user_name: info.user_name.as_ref().map(|cow| cow.to_string()),
                host_name: info.host_name.as_ref().map(|cow| cow.to_string()),
                server_version: info.server_version.as_ref().map(|cow| cow.to_string()),
                server_name: info.server_name.as_ref().map(|cow| cow.to_string()),
                sample_spec: info.sample_spec,
                default_sink_name: info.default_sink_name.as_ref().map(|cow| cow.to_string()),
                default_source_name: info.default_source_name.as_ref().map(|cow| cow.to_string()),
                cookie: info.cookie,
                channel_map: info.channel_map,
            }))
            .unwrap();
        });
    }

    fn get_mute(ident: Ident, ctx: &Rc<RefCell<Context>>, tx: Sender<PulseAudioCommandResult>) {
        let introspector = ctx.borrow_mut().introspect();
        cb!(
            ident.clone(),
            introspector,
            get_sink_info_by_,
            move |result: ListResult<&SinkInfo>| {
                if let ListResult::Item(inner) = result {
                    tx.send(PulseAudioCommandResult::Mute(ident.clone(), inner.mute))
                        .unwrap();
                }
            }
        );
    }
}
