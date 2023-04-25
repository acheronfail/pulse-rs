use std::cell::RefCell;
use std::error::Error;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::channelmap::Position;
use libpulse_binding::context::introspect::{SinkInfo, SourceInfo};
use libpulse_binding::context::subscribe::{Facility, InterestMaskSet, Operation};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::proplist::{properties, Proplist};
use libpulse_binding::volume::{Volume, VolumeDB, VolumeLinear};

use super::api::{PACommand, PAEvent, PAIdent, PAServerInfo};
use crate::pulseaudio::api::VolumeReading;
use crate::pulseaudio::util::VolumePercentage;

type Ctx = Rc<RefCell<Context>>;
type Res = Result<(), Box<dyn Error>>;

macro_rules! impl_call {
    ($method:ident, $inner:ty) => {
        fn $method<F>(ident: PAIdent, ctx: &Ctx, mut f: F)
        where
            F: FnMut(PAIdent, $inner) -> Res + 'static,
        {
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
                    if let ListResult::Item(inner) = result {
                        (&mut f)(ident.clone(), inner).unwrap();
                    }
                });
            }
        }
    };
}

// fn get_sink_info<F>(ident: PulseAudioId, ctx: &Ctx, mut f: F)
// where
//     F: FnMut(PulseAudioId, &SinkInfo) -> Res + 'static,
// {
//     cb!(ident, ctx, get_sink_info_by_, move |result| {
//         if let ListResult::Item(inner) = result {
//             (&mut f)(ident.clone(), inner).unwrap();
//         }
//     });
// }

pub struct PulseAudio;

impl PulseAudio {
    // https://freedesktop.org/software/pulseaudio/doxygen/threaded_mainloop.html
    // https://gavv.net/articles/pulseaudio-under-the-hood/#asynchronous-api
    // https://docs.rs/libpulse-binding/2.26.0/libpulse_binding/mainloop/threaded/index.html#example
    pub fn connect(
        with_app_name: impl AsRef<str>,
        cmd_rx: Receiver<PACommand>,
        result_tx: Sender<PAEvent>,
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

            let tx = result_tx.clone();
            ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
                move |facility, operation, index| {
                    // SAFETY: as per libpulse_binding's documentation, this should be safe
                    let operation = operation.unwrap();
                    let kind = facility.unwrap();

                    let id = PAIdent::Index(index);
                    match operation {
                        Operation::New => tx.send(PAEvent::New(kind, id)).unwrap(),
                        Operation::Removed => tx.send(PAEvent::Removed(kind, id)).unwrap(),
                        Operation::Changed => tx.send(PAEvent::Changed(kind, id)).unwrap(),
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

    fn handle_cmd(cmd: PACommand, ctx: &Ctx, result_tx: &Sender<PAEvent>) {
        let tx = result_tx.clone();
        match cmd {
            PACommand::GetServerInfo => Self::get_server_info(ctx, tx),
            PACommand::GetMute(kind, id) => match kind {
                Facility::Sink => Self::get_sink_mute(id, ctx, tx),
                Facility::Source => Self::get_source_mute(id, ctx, tx),
                _ => todo!(),
            },
            PACommand::GetVolume(kind, id) => match kind {
                Facility::Sink => Self::get_sink_volume(id, ctx, tx),
                Facility::Source => Self::get_source_volume(id, ctx, tx),
                _ => todo!(),
            },
            _ => todo!(),
        }
    }

    fn get_server_info(ctx: &Ctx, tx: Sender<PAEvent>) {
        let introspector = ctx.borrow_mut().introspect();
        introspector.get_server_info(move |info| {
            tx.send(PAEvent::ServerInfo(PAServerInfo {
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

    impl_call!(get_sink_info, &SinkInfo);
    impl_call!(get_source_info, &SourceInfo);

    fn get_sink_mute(ident: PAIdent, ctx: &Ctx, tx: Sender<PAEvent>) {
        Self::get_sink_info(ident, ctx, move |ident, info| {
            tx.send(PAEvent::Mute(ident, info.mute))?;
            Ok(())
        });
    }

    fn get_source_mute(ident: PAIdent, ctx: &Ctx, tx: Sender<PAEvent>) {
        Self::get_source_info(ident, ctx, move |ident, info| {
            tx.send(PAEvent::Mute(ident, info.mute))?;
            Ok(())
        });
    }

    fn get_sink_volume(ident: PAIdent, ctx: &Ctx, tx: Sender<PAEvent>) {
        Self::get_sink_info(ident, ctx, move |ident, info| {
            tx.send(PAEvent::Volume(
                ident,
                Self::read_volumes(
                    info.channel_map.get().into_iter(),
                    info.volume.get().into_iter(),
                ),
            ))?;
            Ok(())
        });
    }

    fn get_source_volume(ident: PAIdent, ctx: &Ctx, tx: Sender<PAEvent>) {
        Self::get_source_info(ident, ctx, move |ident, info| {
            tx.send(PAEvent::Volume(
                ident,
                Self::read_volumes(
                    info.channel_map.get().into_iter(),
                    info.volume.get().into_iter(),
                ),
            ))?;
            Ok(())
        });
    }

    fn read_volumes<'a>(
        channels: impl Iterator<Item = &'a Position>,
        volumes: impl Iterator<Item = &'a Volume>,
    ) -> Vec<VolumeReading> {
        channels
            .zip(volumes)
            .map(|(chan, vol)| VolumeReading {
                channel: *chan,
                percentage: VolumePercentage::from(*vol).0,
                linear: VolumeLinear::from(*vol).0,
                value: vol.0,
                db: VolumeDB::from(*vol).0,
            })
            .collect()
    }
}
