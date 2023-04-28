use serde::Serialize;

use super::*;
use crate::sender::EventSender;

#[derive(Debug)]
pub enum PACommand {
    GetServerInfo,

    GetSinkInfoList,
    GetSinkMute(PAIdent),
    GetSinkVolume(PAIdent),
    SetSinkMute(PAIdent, bool),
    SetSinkVolume(PAIdent, VolumeSpec),

    GetSourceInfoList,
    GetSourceMute(PAIdent),
    GetSourceVolume(PAIdent),
    SetSourceMute(PAIdent, bool),
    SetSourceVolume(PAIdent, VolumeSpec),

    GetCardInfoList,
    GetClientInfoList,
    GetModuleInfoList,
    GetSampleInfoList,
    GetSinkInputInfoList,
    GetSourceOutputInfoList,

    Subscribe(PAMask, Box<dyn EventSender>),

    Disconnect,
    // TODO: sink inputs & source outputs (mute & volume)
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PAEvent {
    // Subscription events
    SubscriptionNew(PAFacility, PAIdent),
    SubscriptionRemoved(PAFacility, PAIdent),
    SubscriptionChanged(PAFacility, PAIdent),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PAResponse {
    /// Generic error event
    Error(String),
    /// Generic operation event
    Complete,
    /// `PACommand::GetServerInfo` response
    ServerInfo(PAServerInfo),
    /// `PACommand::GetSinkInfoList` response
    SinkInfoList(Vec<PASinkInfo>),
    /// `PACommand::GetSourceInfoList` response
    SourceInfoList(Vec<PASourceInfo>),
    /// `PACommand::GetSinkInputList` response
    SinkInputInfoList(Vec<PASinkInputInfo>),
    /// `PACommand::GetSourceOutputList` response
    SourceOutputInfoList(Vec<PASourceOutputInfo>),
    /// `PACommand::GetClientInfoList` response
    ClientInfoList(Vec<PAClientInfo>),
    /// `PACommand::SampleInfoList` response
    SampleInfoList(Vec<PASampleInfo>),
    /// `PACommand::CardInfoList` response
    CardInfoList(Vec<PACardInfo>),
    /// `PACommand::ModuleInfoList` response
    ModuleInfoList(Vec<PAModuleInfo>),
    /// `PACommand::Get*Mute` response
    Mute(PAIdent, bool),
    /// `PACommand::Get*Volume` response
    Volume(PAIdent, VolumeReadings),
    /// `PACommand::Disconnect` response.
    /// Once this is received, no other `PACommand`s should be sent, since the
    /// receiver will have been dropped.
    Disconnected,
}
