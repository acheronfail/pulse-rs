use libpulse_binding::volume::{ChannelVolumes, Volume};
use libpulse_sys::pa_cvolume;

#[derive(Debug)]
pub struct VolumePercentage(pub f64);

impl Into<ChannelVolumes> for VolumePercentage {
    fn into(self) -> ChannelVolumes {
        let pct = self.0.clamp(0.0, 1.5);
        // libpulse doesn't seem to offer a way to calculate percentages...
        let v = (Volume::NORMAL.0 as f64 * pct) as u32;
        // is this really the only way to create a `ChannelVolumes`?
        let mut inner = pa_cvolume::default();
        inner.channels = 2;
        inner.values[0] = v;
        inner.values[1] = v;
        inner.into()
    }
}
