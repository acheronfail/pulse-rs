use std::error::Error;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use serde::Serialize;

use crate::api::*;
use crate::mainloop::PulseAudioLoop;
use crate::sender::EventSender;

macro_rules! extract_unsafe {
    ($event:expr, $pattern:pat => $mapping:expr) => {
        match $event {
            $pattern => Ok($mapping),
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

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

// TODO: docs on when disconnect occurs
pub struct PulseAudio {
    tx: Sender<PACommand>,
    rx_resp: Receiver<PAResponse>,
}

impl PulseAudio {
    pub const DEFAULT_NAME: &str = "Pulser";

    pub fn connect(name: Option<&str>) -> PulseAudio {
        let name = name
            .map(|s| s.as_ref())
            .unwrap_or(Self::DEFAULT_NAME)
            .to_owned();

        let (tx, rx_resp) = PulseAudioLoop::start(name);
        PulseAudio { tx, rx_resp }
    }

    pub fn subscribe(&self, mask: PAMask, tx: Box<dyn EventSender>) -> Result<OperationResult> {
        self.tx.send(PACommand::Subscribe(mask, tx))?;
        self.operation_result()
    }

    pub fn get_server_info(&self) -> Result<PAServerInfo> {
        self.tx.send(PACommand::GetServerInfo)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::ServerInfo(x) => x)
    }

    pub fn get_card_info_list(&self) -> Result<Vec<PACardInfo>> {
        self.tx.send(PACommand::GetCardInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::CardInfoList(x) => x)
    }

    pub fn get_client_info_list(&self) -> Result<Vec<PAClientInfo>> {
        self.tx.send(PACommand::GetClientInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::ClientInfoList(x) => x)
    }

    pub fn get_module_info_list(&self) -> Result<Vec<PAModuleInfo>> {
        self.tx.send(PACommand::GetModuleInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::ModuleInfoList(x) => x)
    }

    pub fn get_sample_info_list(&self) -> Result<Vec<PASampleInfo>> {
        self.tx.send(PACommand::GetSampleInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::SampleInfoList(x) => x)
    }

    pub fn get_sink_info_list(&self) -> Result<Vec<PASinkInfo>> {
        self.tx.send(PACommand::GetSinkInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::SinkInfoList(x) => x)
    }

    pub fn get_sink_input_info_list(&self) -> Result<Vec<PASinkInputInfo>> {
        self.tx.send(PACommand::GetSinkInputInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::SinkInputInfoList(x) => x)
    }

    pub fn get_source_info_list(&self) -> Result<Vec<PASourceInfo>> {
        self.tx.send(PACommand::GetSourceInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::SourceInfoList(x) => x)
    }

    pub fn get_source_output_info_list(&self) -> Result<Vec<PASourceOutputInfo>> {
        self.tx.send(PACommand::GetSourceOutputInfoList)?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::SourceOutputInfoList(x) => x)
    }

    pub fn get_sink_mute(&self, id: PAIdent) -> Result<bool> {
        self.tx.send(PACommand::GetSinkMute(id))?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::Mute(_, x) => x)
    }

    pub fn get_sink_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        self.tx.send(PACommand::GetSinkVolume(id))?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::Volume(_, x) => x)
    }

    pub fn set_sink_mute(&self, id: PAIdent, mute: bool) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSinkMute(id, mute))?;
        self.operation_result()
    }

    pub fn set_sink_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSinkVolume(id, vol))?;
        self.operation_result()
    }

    pub fn get_source_mute(&self, id: PAIdent) -> Result<bool> {
        self.tx.send(PACommand::GetSourceMute(id))?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::Mute(_, x) => x)
    }

    pub fn get_source_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        self.tx.send(PACommand::GetSourceVolume(id))?;
        extract_unsafe!(self.rx_resp.recv()?, PAResponse::Volume(_, x) => x)
    }

    pub fn set_source_mute(&self, id: PAIdent, mute: bool) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSourceMute(id, mute))?;
        self.operation_result()
    }

    pub fn set_source_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<OperationResult> {
        self.tx.send(PACommand::SetSourceVolume(id, vol))?;
        self.operation_result()
    }

    fn operation_result(&self) -> Result<OperationResult> {
        match self.rx_resp.recv()? {
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
        match self.rx_resp.recv_timeout(Duration::from_secs(3)) {
            Ok(PAResponse::Disconnected) => {}
            Ok(ev) => unreachable!("Unexpected event: {:?}", ev),
            Err(RecvTimeoutError::Disconnected) => todo!("handle sender dropped"),
            Err(RecvTimeoutError::Timeout) => todo!("response timed out"),
        }
    }
}
