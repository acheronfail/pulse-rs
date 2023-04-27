use libpulse_binding::channelmap::{Map, Position};
use libpulse_binding::volume::{ChannelVolumes, Volume};
use libpulse_sys::{pa_channel_map, pa_cvolume};

use super::api::{VolumeReadings, VolumeSpec};

pub fn new_channel_volumes(volumes: Vec<Volume>) -> ChannelVolumes {
    let mut inner = pa_cvolume::default();
    inner.channels = volumes.len() as u8;
    for (i, vol) in volumes.into_iter().enumerate() {
        inner.values[i] = vol.0;
    }

    // is this really the only way to create a `ChannelVolumes`?
    inner.into()
}

pub fn updated_channel_volumes(
    current: ChannelVolumes,
    volume_spec: &VolumeSpec,
) -> ChannelVolumes {
    match volume_spec {
        VolumeSpec::All(vol) => {
            let mut cv = current.clone();
            cv.set(current.len(), (*vol).into());
            cv
        }
        VolumeSpec::Channels(vols) => {
            let volumes: Vec<Volume> = vols.into_iter().map(|v| (*v).into()).collect();
            // TODO: return an error here, rather than asserting
            assert!(
                volumes.len() as u8 == current.len(),
                "Failed to set volumes. Provided channel count: {}, actual count: {}",
                volumes.len(),
                current.len()
            );
            new_channel_volumes(volumes)
        }
    }
}

pub fn new_channel_map(channels: Vec<Position>) -> Map {
    let mut inner = pa_channel_map::default();
    inner.channels = channels.len() as u8;
    for (i, chan) in channels.into_iter().enumerate() {
        inner.map[i] = chan.into();
    }

    // is this really the only way to create a `Map`?
    inner.into()
}

impl From<VolumeReadings> for ChannelVolumes {
    fn from(value: VolumeReadings) -> Self {
        new_channel_volumes(value.inner.into_iter().map(|v| v.volume.0).collect())
    }
}
