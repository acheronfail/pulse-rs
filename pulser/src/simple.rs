use std::error::Error;
use std::fmt::Display;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use serde::Serialize;

use crate::api::*;
use crate::mainloop::PulseAudioLoop;
use crate::sender::EventSender;

macro_rules! assume_variant {
    ($event:expr, $pattern:pat => $mapping:expr) => {
        match $event {
            $pattern => Ok($mapping),
            PAResponse::OpError(s) => Err((OperationResult::Failure { error: s }).into()),
            ev => Err(format!("Expected {} but received {:?}", stringify!($pattern), ev).into()),
        }
    };
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OperationResult {
    Success,
    Failure { error: String },
}

impl Display for OperationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationResult::Success => f.write_str("success"),
            OperationResult::Failure { error } => f.write_str(error),
        }
    }
}

impl Error for OperationResult {}

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

// TODO: docs on when disconnect occurs
pub struct PulseAudio {
    tx: Sender<PACommand>,
    rx: Receiver<PAResponse>,
}

macro_rules! impl_find {
    ($ty:ident) => {
        paste::paste! {
            fn [<find_ $ty:snake _by_name>](&self, name: &String) -> Result<[<PA $ty>]> {
                let items = self.[<get_ $ty:snake _list>]()?;
                items
                    .into_iter()
                    .find(|x| x.name.as_ref() == Some(name))
                    .ok_or_else(|| {
                        format!("No {} found with name: {}", stringify!([<$ty:snake>]), name).into()
                    })
            }
        }
    };
}

impl PulseAudio {
    pub const DEFAULT_NAME: &str = "Pulser";

    impl_find!(ClientInfo);
    impl_find!(ModuleInfo);
    impl_find!(SinkInputInfo);
    impl_find!(SourceOutputInfo);

    pub fn connect(name: Option<&str>) -> PulseAudio {
        let name = name
            .map(|s| s.as_ref())
            .unwrap_or(Self::DEFAULT_NAME)
            .to_owned();

        let (tx, rx) = PulseAudioLoop::start(name);
        PulseAudio { tx, rx }
    }

    /*
     * Server
     */

    pub fn get_server_info(&self) -> Result<PAServerInfo> {
        self.tx.send(PACommand::GetServerInfo)?;
        assume_variant!(self.rx.recv()?, PAResponse::ServerInfo(x) => x)
    }

    pub fn get_default_sink(&self) -> Result<Option<PAIdent>> {
        self.tx.send(PACommand::GetDefaultSink)?;
        assume_variant!(self.rx.recv()?, PAResponse::DefaultSink(x) => x)
    }

    pub fn set_default_sink(&self, id: PAIdent) -> Result<OperationResult> {
        self.tx.send(PACommand::SetDefaultSink(id))?;
        self.operation_result()
    }

    pub fn get_default_source(&self) -> Result<Option<PAIdent>> {
        self.tx.send(PACommand::GetDefaultSource)?;
        assume_variant!(self.rx.recv()?, PAResponse::DefaultSource(x) => x)
    }

    pub fn set_default_source(&self, id: PAIdent) -> Result<OperationResult> {
        self.tx.send(PACommand::SetDefaultSource(id))?;
        self.operation_result()
    }

    /*
     * Subscriptions
     */

    pub fn subscribe(&self, mask: PAMask, tx: Box<dyn EventSender>) -> Result<OperationResult> {
        self.tx.send(PACommand::Subscribe(mask, tx))?;
        self.operation_result()
    }

    /*
     * Cards
     */

    pub fn get_card_info(&self, id: PAIdent) -> Result<PACardInfo> {
        self.tx.send(PACommand::GetCardInfo(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::CardInfo(x) => x)
    }

    pub fn set_card_profile(&self, id: PAIdent, profile: String) -> Result<OperationResult> {
        self.tx.send(PACommand::SetCardProfile(id, profile))?;
        self.operation_result()
    }

    pub fn set_port_latency_offset(
        &self,
        card_id: PAIdent,
        port_id: PAIdent,
        offset: i64,
    ) -> Result<OperationResult> {
        let card = self.get_card_info(card_id.clone())?;
        let port = card.ports.into_iter().enumerate().find_map(|(i, p)| {
            let found = match port_id {
                PAIdent::Index(idx) => idx == i as u32,
                PAIdent::Name(ref name) => p.name.as_ref() == Some(name),
            };

            if found {
                Some(p)
            } else {
                None
            }
        });
        let card = match card.name {
            Some(name) => name,
            None => return Err(format!("No card found with id: {}", card_id).into()),
        };
        let port = match port {
            Some(port) => match port.name {
                Some(name) => name,
                None => {
                    return Err(format!(
                        "Found port with id: {}, but it has no name and one is required",
                        port_id
                    )
                    .into())
                }
            },
            None => return Err(format!("No port found with id: {}", port_id).into()),
        };

        self.tx
            .send(PACommand::SetPortLatencyOffset(card, port, offset))?;
        self.operation_result()
    }

    /*
     * Clients
     */

    pub fn get_client_info(&self, id: PAIdent) -> Result<PAClientInfo> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetClientInfo(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::ClientInfo(x) => x)
            }
            PAIdent::Name(ref name) => {
                let client = self.find_client_info_by_name(name)?;
                self.get_client_info(PAIdent::Index(client.index))
            }
        }
    }

    pub fn kill_client(&self, id: PAIdent) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::KillClient(idx))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let client = self.find_client_info_by_name(name)?;
                self.kill_client(PAIdent::Index(client.index))
            }
        }
    }

    /*
     * Modules
     */

    pub fn get_module_info(&self, id: PAIdent) -> Result<PAModuleInfo> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetModuleInfo(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::ModuleInfo(x) => x)
            }
            PAIdent::Name(ref name) => {
                let module = self.find_module_info_by_name(name)?;
                self.get_module_info(PAIdent::Index(module.index))
            }
        }
    }

    pub fn load_module(&self, name: String, args: String) -> Result<u32> {
        self.tx.send(PACommand::LoadModule(name, args))?;
        assume_variant!(self.rx.recv()?, PAResponse::ModuleLoaded(x) => x)
    }

    pub fn unload_module(&self, id: PAIdent) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::UnloadModule(idx))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let module = self.find_module_info_by_name(name)?;
                self.unload_module(PAIdent::Index(module.index))
            }
        }
    }

    /*
     * Lists
     */

    pub fn get_card_info_list(&self) -> Result<Vec<PACardInfo>> {
        self.tx.send(PACommand::GetCardInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::CardInfoList(x) => x)
    }

    pub fn get_client_info_list(&self) -> Result<Vec<PAClientInfo>> {
        self.tx.send(PACommand::GetClientInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::ClientInfoList(x) => x)
    }

    pub fn get_module_info_list(&self) -> Result<Vec<PAModuleInfo>> {
        self.tx.send(PACommand::GetModuleInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::ModuleInfoList(x) => x)
    }

    pub fn get_sample_info_list(&self) -> Result<Vec<PASampleInfo>> {
        self.tx.send(PACommand::GetSampleInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::SampleInfoList(x) => x)
    }

    pub fn get_sink_info_list(&self) -> Result<Vec<PASinkInfo>> {
        self.tx.send(PACommand::GetSinkInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::SinkInfoList(x) => x)
    }

    pub fn get_sink_input_info_list(&self) -> Result<Vec<PASinkInputInfo>> {
        self.tx.send(PACommand::GetSinkInputInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::SinkInputInfoList(x) => x)
    }

    pub fn get_source_info_list(&self) -> Result<Vec<PASourceInfo>> {
        self.tx.send(PACommand::GetSourceInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::SourceInfoList(x) => x)
    }

    pub fn get_source_output_info_list(&self) -> Result<Vec<PASourceOutputInfo>> {
        self.tx.send(PACommand::GetSourceOutputInfoList)?;
        assume_variant!(self.rx.recv()?, PAResponse::SourceOutputInfoList(x) => x)
    }

    /*
     * Sinks
     */

    pub fn get_sink_info(&self, id: PAIdent) -> Result<PASinkInfo> {
        self.tx.send(PACommand::GetSinkInfo(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::SinkInfo(x) => x)
    }

    pub fn get_sink_mute(&self, id: PAIdent) -> Result<bool> {
        self.tx.send(PACommand::GetSinkMute(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::Mute(_, x) => x)
    }

    pub fn get_sink_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        self.tx.send(PACommand::GetSinkVolume(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::Volume(_, x) => x)
    }

    pub fn set_sink_mute(&self, id: PAIdent, mute: bool) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSinkMute(id, mute))?;
        self.operation_result()
    }

    pub fn set_sink_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSinkVolume(id, vol))?;
        self.operation_result()
    }

    pub fn set_sink_port(&self, id: PAIdent, port: String) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSinkPort(id, port))?;
        self.operation_result()
    }

    pub fn suspend_sink(&self, id: PAIdent, suspend: bool) -> Result<OperationResult> {
        self.tx.send(PACommand::SuspendSink(id, suspend))?;
        self.operation_result()
    }

    /*
     * Sources
     */

    pub fn get_source_info(&self, id: PAIdent) -> Result<PASourceInfo> {
        self.tx.send(PACommand::GetSourceInfo(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::SourceInfo(x) => x)
    }

    pub fn get_source_mute(&self, id: PAIdent) -> Result<bool> {
        self.tx.send(PACommand::GetSourceMute(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::Mute(_, x) => x)
    }

    pub fn get_source_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        self.tx.send(PACommand::GetSourceVolume(id))?;
        assume_variant!(self.rx.recv()?, PAResponse::Volume(_, x) => x)
    }

    pub fn set_source_mute(&self, id: PAIdent, mute: bool) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSourceMute(id, mute))?;
        self.operation_result()
    }

    pub fn set_source_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSourceVolume(id, vol))?;
        self.operation_result()
    }

    pub fn set_source_port(&self, id: PAIdent, port: String) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSourcePort(id, port))?;
        self.operation_result()
    }

    pub fn suspend_source(&self, id: PAIdent, suspend: bool) -> Result<OperationResult> {
        self.tx.send(PACommand::SuspendSource(id, suspend))?;
        self.operation_result()
    }

    /*
     * Sink Inputs
     */

    pub fn get_sink_input_info(&self, id: PAIdent) -> Result<PASinkInputInfo> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetSinkInputInfo(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::SinkInputInfo(x) => x)
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.get_sink_input_info(PAIdent::Index(si.index))
            }
        }
    }

    pub fn get_sink_input_mute(&self, id: PAIdent) -> Result<bool> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetSinkInputMute(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::Mute(_, x) => x)
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.get_sink_input_mute(PAIdent::Index(si.index))
            }
        }
    }

    pub fn get_sink_input_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetSinkInputVolume(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::Volume(_, x) => x)
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.get_sink_input_volume(PAIdent::Index(si.index))
            }
        }
    }

    pub fn set_sink_input_mute(&self, id: PAIdent, mute: bool) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::SetSinkInputMute(idx, mute))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.set_sink_input_mute(PAIdent::Index(si.index), mute)
            }
        }
    }

    pub fn set_sink_input_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::SetSinkInputVolume(idx, vol))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.set_sink_input_volume(PAIdent::Index(si.index), vol)
            }
        }
    }

    pub fn move_sink_input(&self, id: PAIdent, sink: PAIdent) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::MoveSinkInput(idx, sink))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.move_sink_input(PAIdent::Index(si.index), sink)
            }
        }
    }

    pub fn kill_sink_input(&self, id: PAIdent) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::KillSinkInput(idx))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_sink_input_info_by_name(name)?;
                self.kill_sink_input(PAIdent::Index(si.index))
            }
        }
    }

    /*
     * Source Outputs
     */

    pub fn get_source_output_info(&self, id: PAIdent) -> Result<PASourceOutputInfo> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetSourceOutputInfo(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::SourceOutputInfo(x) => x)
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.get_source_output_info(PAIdent::Index(si.index))
            }
        }
    }

    pub fn get_source_output_mute(&self, id: PAIdent) -> Result<bool> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetSinkInputMute(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::Mute(_, x) => x)
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.get_source_output_mute(PAIdent::Index(si.index))
            }
        }
    }

    pub fn get_source_output_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::GetSinkInputVolume(idx))?;
                assume_variant!(self.rx.recv()?, PAResponse::Volume(_, x) => x)
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.get_source_output_volume(PAIdent::Index(si.index))
            }
        }
    }

    pub fn set_source_output_mute(&self, id: PAIdent, mute: bool) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::SetSinkInputMute(idx, mute))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.set_source_output_mute(PAIdent::Index(si.index), mute)
            }
        }
    }

    pub fn set_source_output_volume(
        &self,
        id: PAIdent,
        vol: VolumeSpec,
    ) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::SetSinkInputVolume(idx, vol))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.set_source_output_volume(PAIdent::Index(si.index), vol)
            }
        }
    }

    pub fn move_source_output(&self, id: PAIdent, source: PAIdent) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::MoveSourceOutput(idx, source))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.move_source_output(PAIdent::Index(si.index), source)
            }
        }
    }

    pub fn kill_source_output(&self, id: PAIdent) -> Result<OperationResult> {
        match id {
            PAIdent::Index(idx) => {
                self.tx.send(PACommand::KillSinkInput(idx))?;
                self.operation_result()
            }
            PAIdent::Name(ref name) => {
                let si = self.find_source_output_info_by_name(name)?;
                self.kill_source_output(PAIdent::Index(si.index))
            }
        }
    }

    /*
     * Util
     */

    fn operation_result(&self) -> Result<OperationResult> {
        match self.rx.recv()? {
            PAResponse::OpComplete => Ok(OperationResult::Success),
            PAResponse::OpError(e) => Ok(OperationResult::Failure { error: e }),
            ev => Err(format!("Unexpected response received {:?}", ev).into()),
        }
    }
}

impl Drop for PulseAudio {
    fn drop(&mut self) {
        // TODO: handle unwraps gracefully
        self.tx.send(PACommand::Disconnect).unwrap();
        match self.rx.recv_timeout(Duration::from_secs(3)) {
            Ok(PAResponse::Disconnected) => {}
            Ok(ev) => unreachable!("Unexpected event: {:?}", ev),
            Err(RecvTimeoutError::Disconnected) => todo!("handle sender dropped"),
            Err(RecvTimeoutError::Timeout) => todo!("response timed out"),
        }
    }
}
