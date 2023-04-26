use libpulse_binding::context::subscribe::Facility;

use super::{PAIdent, PAServerInfo, PASinkInfo, PASourceInfo, VolumeReadings, VolumeSpec};

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
    /// `PACommand::ListSinks` response
    SinkInfoList(Vec<PASinkInfo>),
    /// `PACommand::ListSinks` response
    SourceInfoList(Vec<PASourceInfo>),
    /// `PACommand::GetServerInfo` response
    ServerInfo(PAServerInfo),
    /// `PACommand::Get*Mute` response
    Mute(PAIdent, bool),
    /// `PACommand::Get*Volume` response
    Volume(PAIdent, VolumeReadings),
}
