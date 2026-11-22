//! Core Audio (default playback device master volume). Compiled by Windows CI only; not run on a real PC yet.
//!
//! `IAudioEndpointVolume` reads and writes the same master level and mute flag as the Windows volume slider.
//! Changes made elsewhere arrive through `IAudioEndpointVolumeCallback`; a changed default device arrives
//! through `IMMNotificationClient`, and we re-bind to the new one. Both callbacks only post a message to the
//! volume thread, which does the actual work.

use std::sync::mpsc::Sender;
use vex_core::volume::{percent_to_scalar, unmutes_on_set, VolumeState};
use windows::{
    core::{implement, Result as WinResult, PCWSTR},
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            eMultimedia, eRender,
            Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl},
            EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator,
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    },
};

use crate::volume::Msg;

pub fn init_com_for_this_thread() {
    // Multithreaded apartment: the callbacks arrive on Windows' own threads and only post a message.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    tx: Sender<Msg>,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, _data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> WinResult<()> {
        let _ = self.tx.send(Msg::Changed);
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceNotifications {
    tx: Sender<Msg>,
}

impl IMMNotificationClient_Impl for DeviceNotifications_Impl {
    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _state: DEVICE_STATE) -> WinResult<()> {
        let _ = self.tx.send(Msg::DeviceChanged);
        Ok(())
    }
    fn OnDeviceAdded(&self, _id: &PCWSTR) -> WinResult<()> {
        let _ = self.tx.send(Msg::DeviceChanged);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> WinResult<()> {
        let _ = self.tx.send(Msg::DeviceChanged);
        Ok(())
    }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _id: &PCWSTR) -> WinResult<()> {
        if flow == eRender && role == eMultimedia {
            let _ = self.tx.send(Msg::DeviceChanged);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> WinResult<()> {
        Ok(())
    }
}

struct Bound {
    endpoint: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl Drop for Bound {
    fn drop(&mut self) {
        unsafe {
            let _ = self.endpoint.UnregisterControlChangeNotify(&self.callback);
        }
    }
}

pub struct Audio {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
    bound: Option<Bound>,
    tx: Sender<Msg>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        unsafe {
            let _ = self.enumerator.UnregisterEndpointNotificationCallback(&self.client);
        }
    }
}

impl Audio {
    pub fn new(tx: Sender<Msg>) -> Result<Self, String> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("audio enumerator: {e}"))?;
            let client: IMMNotificationClient = DeviceNotifications { tx: tx.clone() }.into();
            enumerator
                .RegisterEndpointNotificationCallback(&client)
                .map_err(|e| format!("device notifications: {e}"))?;
            let mut audio = Audio { enumerator, client, bound: None, tx };
            audio.rebind();
            Ok(audio)
        }
    }

    /// Bind to the current default playback device (or to nothing when there is none).
    pub fn rebind(&mut self) {
        self.bound = None; // unregisters the old callback
        self.bound = self.bind().ok();
    }

    fn bind(&self) -> WinResult<Bound> {
        unsafe {
            let device = self.enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            let callback: IAudioEndpointVolumeCallback = VolumeCallback { tx: self.tx.clone() }.into();
            endpoint.RegisterControlChangeNotify(&callback)?;
            Ok(Bound { endpoint, callback })
        }
    }

    pub fn state(&self) -> VolumeState {
        let Some(b) = &self.bound else {
            return VolumeState::default();
        };
        unsafe {
            match (b.endpoint.GetMasterVolumeLevelScalar(), b.endpoint.GetMute()) {
                (Ok(level), Ok(muted)) => VolumeState::from_scalar(level, muted.as_bool(), None),
                _ => VolumeState::unavailable("Could not read the volume"),
            }
        }
    }

    pub fn set_percent(&self, percent: i64) -> Result<(), String> {
        let b = self.bound.as_ref().ok_or("No audio output device")?;
        let clamped = percent.clamp(0, 100) as u8;
        unsafe {
            b.endpoint
                .SetMasterVolumeLevelScalar(percent_to_scalar(percent), std::ptr::null())
                .map_err(|_| "Could not change the volume".to_string())?;
            if unmutes_on_set(clamped) && b.endpoint.GetMute().map(|m| m.as_bool()).unwrap_or(false) {
                b.endpoint.SetMute(false, std::ptr::null()).map_err(|_| "Could not unmute".to_string())?;
            }
        }
        Ok(())
    }

    pub fn toggle_mute(&self) -> Result<(), String> {
        let b = self.bound.as_ref().ok_or("No audio output device")?;
        unsafe {
            let muted = b.endpoint.GetMute().map_err(|_| "Could not read the mute state".to_string())?.as_bool();
            b.endpoint.SetMute(!muted, std::ptr::null()).map_err(|_| "Could not change mute".to_string())
        }
    }
}
