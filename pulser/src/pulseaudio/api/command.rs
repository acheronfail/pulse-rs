use serde::Serialize;

use super::*;
use crate::sender::EventSender;

#[derive(Debug)]
pub enum PACommand {
    GetServerInfo,

    GetDefaultSink,
    GetDefaultSource,
    SetDefaultSink(PAIdent),
    SetDefaultSource(PAIdent),

    GetSinkMute(PAIdent),
    GetSinkVolume(PAIdent),
    SetSinkMute(PAIdent, bool),
    SetSinkVolume(PAIdent, VolumeSpec),

    GetSourceMute(PAIdent),
    GetSourceVolume(PAIdent),
    SetSourceMute(PAIdent, bool),
    SetSourceVolume(PAIdent, VolumeSpec),

    GetSinkInputMute(u32),
    GetSinkInputVolume(u32),
    SetSinkInputMute(u32, bool),
    SetSinkInputVolume(u32, VolumeSpec),

    GetSourceOutputMute(u32),
    GetSourceOutputVolume(u32),
    SetSourceOutputMute(u32, bool),
    SetSourceOutputVolume(u32, VolumeSpec),

    // TODO: "get by name/index" for each of these
    GetCardInfoList,
    GetClientInfoList,
    GetModuleInfoList,
    GetSampleInfoList,
    GetSinkInfoList,
    GetSinkInputInfoList,
    GetSourceInfoList,
    GetSourceOutputInfoList,

    Subscribe(PAMask, Box<dyn EventSender>),

    Disconnect,
    // TODO: move sink-input/source-output
    // TODO: set sink/source port
    // TODO: load/unload module
    // TODO: send message
    // TODO: set card profile
    // TODO: set port latency offset
    // TODO: kill sink-input/source-output/client
    // TODO: suspend sink/source
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
    /// `PACommand::GetDefaultSink` response
    DefaultSink(Option<PAIdent>),
    /// `PACommand::GetDefaultSource` response
    DefaultSource(Option<PAIdent>),
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
