use libpulse_binding::context::subscribe::Facility;

use super::*;

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
    GetSinkInputList,
    GetSourceOutputList,
    // TODO: sink inputs & source outputs (mute & volume)
    // TODO: modules
    // TODO: cards
    // TODO: list
    // TODO: info for everything
}

#[derive(Debug)]
pub enum PAEvent {
    /// Generic error event
    Error(String),
    /// Generic operation event
    Complete(bool),
    // Subscription events
    SubscriptionNew(Facility, PAIdent),
    SubscriptionRemoved(Facility, PAIdent),
    SubscriptionChanged(Facility, PAIdent),
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
}
