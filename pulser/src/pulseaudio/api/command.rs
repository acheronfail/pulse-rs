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
/// Subscription events
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PAEvent {
    SubscriptionNew(PAFacility, PAIdent),
    SubscriptionRemoved(PAFacility, PAIdent),
    SubscriptionChanged(PAFacility, PAIdent),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PAResponse {
    /// Returned when an operation succeeded (such as setting mute/volume, or starting a subscription)
    OpComplete,
    /// Returned when an operation failed (such as setting mute/volume, or starting a subscription)
    OpError(String),

    /// `PACommand::CardInfoList` response
    CardInfoList(Vec<PACardInfo>),
    /// `PACommand::GetClientInfoList` response
    ClientInfoList(Vec<PAClientInfo>),
    /// `PACommand::ModuleInfoList` response
    ModuleInfoList(Vec<PAModuleInfo>),
    /// `PACommand::Get*Mute` response
    Mute(PAIdent, bool),
    /// `PACommand::SampleInfoList` response
    SampleInfoList(Vec<PASampleInfo>),
    /// `PACommand::GetServerInfo` response
    ServerInfo(PAServerInfo),
    /// `PACommand::GetSinkInfoList` response
    SinkInfoList(Vec<PASinkInfo>),
    /// `PACommand::GetSinkInputList` response
    SinkInputInfoList(Vec<PASinkInputInfo>),
    /// `PACommand::GetSourceInfoList` response
    SourceInfoList(Vec<PASourceInfo>),
    /// `PACommand::GetSourceOutputList` response
    SourceOutputInfoList(Vec<PASourceOutputInfo>),
    /// `PACommand::Get*Volume` response
    Volume(PAIdent, VolumeReadings),

    /// `PACommand::Disconnect` response.
    /// Once this is received, no other `PACommand`s should be sent, since the
    /// receiver will have been dropped.
    Disconnected,
}
