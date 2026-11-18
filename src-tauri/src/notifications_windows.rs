//! Windows notifications through `UserNotificationListener`.
//!
//! This is the only Windows-specific notification code. It reads what Windows reports and reduces
//! it to `RawNotification`; all decisions (filtering, de-duplication, history, privacy) live in
//! `vex_core::notifications` where they are unit-tested.
//!
//! Windows limitation (documented by Microsoft, and the reason for `probe`): the listener only
//! works for apps that have package identity. A plain installer-based desktop app can be told
//! "Allowed" and then fail with "Element not found" when it reads. The probe below turns that into
//! an honest `Unavailable` state instead of showing an empty list as if nothing had arrived.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use vex_core::media::{artwork_data_url, windows_ticks_to_epoch_ms};
use vex_core::notifications::{Access, Msg, NotificationSource, RawNotification};
use windows::core::HSTRING;
use windows::Foundation::{Size, TypedEventHandler};
use windows::Storage::Streams::DataReader;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{NotificationKinds, UserNotification};

const MAX_ICON_BYTES: u64 = 512 * 1024;
const ICON_SIZE: f32 = 48.0;

const NO_IDENTITY: &str = "Windows only lets apps with package identity read notifications, and this VEX install does not have it. \
Notifications cannot be shown until VEX is packaged with identity (see docs/CHECKLIST.md).";

pub struct WindowsSource {
    listener: UserNotificationListener,
    /// App id -> encoded logo (or `None` when it has none), so each logo is read once.
    icons: HashMap<String, Option<String>>,
    wake: Sender<Msg>,
    subscribed: bool,
}

impl WindowsSource {
    pub fn new(wake: Sender<Msg>) -> Result<Self, String> {
        let listener = UserNotificationListener::Current().map_err(|e| e.to_string())?;
        Ok(Self {
            listener,
            icons: HashMap::new(),
            wake,
            subscribed: false,
        })
    }

    /// Ask Windows to tell us when notifications change, so we do not have to poll quickly.
    fn subscribe(&mut self) {
        if self.subscribed {
            return;
        }
        let tx = self.wake.clone();
        let handler = TypedEventHandler::new(move |_, _| {
            let _ = tx.send(Msg::Changed);
            Ok(())
        });
        // If the event cannot be registered the slow fallback poll in the engine still works.
        self.subscribed = self.listener.NotificationChanged(&handler).is_ok();
    }

    fn read(&mut self) -> windows::core::Result<Vec<RawNotification>> {
        let list = self
            .listener
            .GetNotificationsAsync(NotificationKinds::Toast)?
            .join()?;
        let mut out = Vec::new();
        for n in list {
            // One malformed notification must never hide the others.
            if let Ok(raw) = self.describe(&n) {
                out.push(raw);
            }
        }
        Ok(out)
    }

    fn describe(&mut self, n: &UserNotification) -> windows::core::Result<RawNotification> {
        let info = n.AppInfo()?;
        let app_id = info
            .AppUserModelId()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let app_name = info
            .DisplayInfo()
            .and_then(|d| d.DisplayName())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let mut text = Vec::new();
        if let Ok(binding) = n
            .Notification()
            .and_then(|x| x.Visual())
            .and_then(|v| v.GetBinding(&HSTRING::from("ToastGeneric")))
        {
            if let Ok(elements) = binding.GetTextElements() {
                for e in elements {
                    if let Ok(t) = e.Text() {
                        text.push(t.to_string());
                    }
                }
            }
        }

        let created_ms = n
            .CreationTime()
            .ok()
            .and_then(|t| windows_ticks_to_epoch_ms(t.UniversalTime))
            .unwrap_or(0);
        let icon = if app_id.is_empty() {
            None
        } else if let Some(cached) = self.icons.get(&app_id) {
            cached.clone()
        } else {
            let icon = read_logo(&info);
            self.icons.insert(app_id.clone(), icon.clone());
            icon
        };

        Ok(RawNotification {
            os_id: n.Id()?,
            app_id,
            app_name,
            text,
            created_ms,
            icon,
        })
    }
}

fn read_logo(info: &windows::ApplicationModel::AppInfo) -> Option<String> {
    let logo = info
        .DisplayInfo()
        .ok()?
        .GetLogo(Size {
            Width: ICON_SIZE,
            Height: ICON_SIZE,
        })
        .ok()?;
    let stream = logo.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_ICON_BYTES {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size as u32).ok()?.join().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    artwork_data_url(&buf)
}

impl NotificationSource for WindowsSource {
    fn request_access(&mut self) -> Access {
        let status = match self.listener.RequestAccessAsync().and_then(|op| op.join()) {
            Ok(s) => s,
            Err(e) => {
                return Access::Unavailable {
                    reason: format!(
                        "Windows would not show its notification permission prompt: {e}"
                    ),
                }
            }
        };
        if status == UserNotificationListenerAccessStatus::Allowed {
            // "Allowed" is not proof the listener works for an app without identity: read once.
            return match self.read() {
                Ok(_) => {
                    self.subscribe();
                    Access::Allowed
                }
                Err(_) => Access::Unavailable {
                    reason: NO_IDENTITY.into(),
                },
            };
        }
        if status == UserNotificationListenerAccessStatus::Denied {
            Access::Denied
        } else {
            // Unspecified: the prompt was dismissed without an answer.
            Access::Denied
        }
    }

    fn snapshot(&mut self) -> Result<Vec<RawNotification>, String> {
        self.read()
            .map_err(|e| format!("Windows could not read notifications: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the real Windows API (CI runs `--ignored` as a separate, non-blocking step).
    /// It records what an installer-based, identity-less app actually gets, so the honest
    /// "unavailable" message is based on observation. It must never panic or hang.
    #[test]
    #[ignore = "needs the real Windows notification APIs; CI runs it as a separate non-blocking step"]
    fn listener_reports_a_clean_state_without_package_identity() {
        crate::media_windows::init_com_for_this_thread();
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut source = WindowsSource::new(tx).expect("the listener object should be creatable");
        let access = source.request_access();
        println!("notification access on this machine: {access:?}");
        match access {
            Access::Allowed => {
                let snap = source.snapshot().expect("allowed access must be readable");
                println!("notifications currently held: {}", snap.len());
            }
            Access::Denied | Access::Unavailable { .. } | Access::Idle => {}
        }
    }
}
