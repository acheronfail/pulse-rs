use std::error::Error;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use crate::api::*;
use crate::mainloop::PulseAudioLoop;

macro_rules! extract_unsafe {
    ($event:expr, $pattern:pat => $mapping:expr) => {
        match $event {
            $pattern => Ok($mapping),
            ev => Err(format!("Expected {} but received {:?}", stringify!($pattern), ev).into()),
        }
    };
}

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

// TODO: docs on when disconnect occurs
pub struct PulseAudio {
    tx: Sender<PACommand>,
    rx: Receiver<PAEvent>,
}

impl PulseAudio {
    pub fn connect() -> PulseAudio {
        let (tx, rx) = PulseAudioLoop::start("pulser");
        PulseAudio { tx, rx }
    }

    pub fn get_server_info(&self) -> Result<PAServerInfo> {
        self.tx.send(PACommand::GetServerInfo)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::ServerInfo(x) => x)
    }

    pub fn get_card_info_list(&self) -> Result<Vec<PACardInfo>> {
        self.tx.send(PACommand::GetCardInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::CardInfoList(x) => x)
    }

    pub fn get_client_info_list(&self) -> Result<Vec<PAClientInfo>> {
        self.tx.send(PACommand::GetClientInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::ClientInfoList(x) => x)
    }

    pub fn get_module_info_list(&self) -> Result<Vec<PAModuleInfo>> {
        self.tx.send(PACommand::GetModuleInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::ModuleInfoList(x) => x)
    }

    pub fn get_sample_info_list(&self) -> Result<Vec<PASampleInfo>> {
        self.tx.send(PACommand::GetSampleInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::SampleInfoList(x) => x)
    }

    pub fn get_sink_info_list(&self) -> Result<Vec<PASinkInfo>> {
        self.tx.send(PACommand::GetSinkInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::SinkInfoList(x) => x)
    }

    pub fn get_sink_input_info_list(&self) -> Result<Vec<PASinkInputInfo>> {
        self.tx.send(PACommand::GetSinkInputInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::SinkInputInfoList(x) => x)
    }

    pub fn get_source_info_list(&self) -> Result<Vec<PASourceInfo>> {
        self.tx.send(PACommand::GetSourceInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::SourceInfoList(x) => x)
    }

    pub fn get_source_output_info_list(&self) -> Result<Vec<PASourceOutputInfo>> {
        self.tx.send(PACommand::GetSourceOutputInfoList)?;
        extract_unsafe!(self.rx.recv()?, PAEvent::SourceOutputInfoList(x) => x)
    }

    pub fn get_sink_mute(&self, id: PAIdent) -> Result<bool> {
        self.tx.send(PACommand::GetSinkMute(id))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Mute(_, x) => x)
    }

    pub fn get_sink_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        self.tx.send(PACommand::GetSinkVolume(id))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Volume(_, x) => x)
    }

    pub fn set_sink_mute(&self, id: PAIdent, mute: bool) -> Result<()> {
        self.tx.send(PACommand::SetSinkMute(id, mute))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Complete => ())
    }

    pub fn set_sink_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<()> {
        self.tx.send(PACommand::SetSinkVolume(id, vol))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Complete => ())
    }

    pub fn get_source_mute(&self, id: PAIdent) -> Result<bool> {
        self.tx.send(PACommand::GetSourceMute(id))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Mute(_, x) => x)
    }

    pub fn get_source_volume(&self, id: PAIdent) -> Result<VolumeReadings> {
        self.tx.send(PACommand::GetSourceVolume(id))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Volume(_, x) => x)
    }

    pub fn set_source_mute(&self, id: PAIdent, mute: bool) -> Result<()> {
        self.tx.send(PACommand::SetSourceMute(id, mute))?;
        extract_unsafe!(self.rx.recv()?, PAEvent::Complete => ())
    }

    pub fn set_source_volume(&self, id: PAIdent, vol: VolumeSpec) -> Result<()> {
        self.tx.send(PACommand::SetSourceVolume(id, vol))?;
        extract_unsafe!(self.rx.recv()?,PAEvent::Complete => ())
    }
}

impl Drop for PulseAudio {
    fn drop(&mut self) {
        // TODO: handle unwraps gracefully
        self.tx.send(PACommand::Disconnect).unwrap();
        match self.rx.recv_timeout(Duration::from_secs(3)) {
            Ok(PAEvent::Disconnected) => {}
            Ok(_) => todo!("handle unexpected"),
            Err(RecvTimeoutError::Disconnected) => todo!("handle sender dropped"),
            Err(RecvTimeoutError::Timeout) => todo!("response timed out"),
        }
    }
}
