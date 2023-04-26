use libpulse_binding::context::subscribe::Facility;

use super::{PAIdent, PAServerInfo, VolumeReadings, VolumeSpec};

#[derive(Debug)]
pub enum PACommand {
    GetServerInfo,

    GetSinkMute(PAIdent),
    GetSinkVolume(PAIdent),
    SetSinkMute(PAIdent, bool),
    SetSinkVolume(PAIdent, VolumeSpec),

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
    // Subscription events
    SubscriptionNew(Facility, PAIdent),
    SubscriptionRemoved(Facility, PAIdent),
    SubscriptionChanged(Facility, PAIdent),

    ServerInfo(PAServerInfo),
    Volume(PAIdent, VolumeReadings),
    Mute(PAIdent, bool),
}
