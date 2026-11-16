//! Settings a company's IT department sets for every machine, read from the
//! Windows registry the way Group Policy and Intune write them.
//!
//! Looked up, first found wins:
//!
//!   1. `HKLM\SOFTWARE\Policies\Mayorana\<App>` (this app only, every user);
//!   2. `HKLM\SOFTWARE\Policies\Mayorana` (every Mayorana app, every user);
//!   3. `HKCU\SOFTWARE\Policies\Mayorana\<App>`;
//!   4. `HKCU\SOFTWARE\Policies\Mayorana`.
//!
//! `<App>` is the app's name as people see it (`PolishCast`). Values:
//!
//! | Value                | Type      | Effect                                              |
//! |----------------------|-----------|-----------------------------------------------------|
//! | `DisableUpdateCheck` | REG_DWORD | `1`: never asks mayorana.ch for a newer version     |
//! | `DisableSelfUpdate`  | REG_DWORD | `1`: says a version is out, but never installs it   |
//! | `DisableTelemetry`   | REG_DWORD | `1`: no usage statistics, whatever the person chose |
//! | `Licence`            | REG_SZ    | the licence number every user of the machine gets   |
//!
//! Read once per process. On macOS and Linux there are none yet: every
//! setting reads as unset.

use std::sync::OnceLock;

/// What IT set. `false` and `None` mean "not set": the app's usual behaviour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Policy {
    pub disable_update_check: bool,
    pub disable_self_update: bool,
    pub disable_telemetry: bool,
    pub licence: Option<String>,
}

/// The words the apps show beside anything IT set.
pub const MANAGED: &str = "Managed by your organization";

impl Policy {
    /// What IT set, in words for the person, e.g. "Usage statistics are off".
    /// Empty when nothing is set.
    pub fn managed(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.disable_update_check {
            out.push("Update check is off");
        } else if self.disable_self_update {
            out.push("Updates are installed by your organization");
        }
        if self.disable_telemetry {
            out.push("Usage statistics are off");
        }
        if self.licence.is_some() {
            out.push("Licence number is set for this computer");
        }
        out
    }
}

static POLICY: OnceLock<Policy> = OnceLock::new();

/// The policy for this app, read on first call. Before [`crate::init`] only
/// the settings for every Mayorana app are read.
pub fn get() -> &'static Policy {
    POLICY.get_or_init(|| read(&|name| registry::lookup(crate::app().map(|a| a.name), name)))
}

/// A registry value as the policy reads it.
// Only the Windows registry makes one outside tests.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Number(u32),
    Text(String),
}

fn read(lookup: &dyn Fn(&str) -> Option<Value>) -> Policy {
    let on = |name: &str| matches!(lookup(name), Some(Value::Number(n)) if n != 0);
    Policy {
        disable_update_check: on("DisableUpdateCheck"),
        disable_self_update: on("DisableSelfUpdate"),
        disable_telemetry: on("DisableTelemetry"),
        licence: match lookup("Licence") {
            Some(Value::Text(t)) => crate::licence::normalise(&t),
            _ => None,
        },
    }
}

#[cfg(windows)]
mod registry {
    use super::Value;

    type Hkey = isize;
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001_u32 as i32 as isize;
    const HKEY_LOCAL_MACHINE: Hkey = 0x8000_0002_u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
    const REG_SZ: u32 = 1;
    const REG_DWORD: u32 = 4;
    const ERROR_SUCCESS: i32 = 0;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: Hkey,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut core::ffi::c_void,
            data_len: *mut u32,
        ) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn get(root: Hkey, key: &str, name: &str) -> Option<Value> {
        let (key, name) = (wide(key), wide(name));
        let mut kind = 0u32;
        let mut buf = vec![0u16; 512];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: `key` and `name` are NUL-terminated UTF-16 buffers that live
        // for the call, and `buf` is writable for `len` bytes, which the API
        // never exceeds; it sets `len` to the bytes written.
        let status = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_DWORD,
                &mut kind,
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        match kind {
            REG_DWORD if len >= 4 => {
                Some(Value::Number(u32::from(buf[0]) | (u32::from(buf[1]) << 16)))
            }
            REG_SZ => {
                let units = (len as usize / 2).min(buf.len());
                let text = String::from_utf16_lossy(&buf[..units]);
                Some(Value::Text(text.trim_end_matches('\0').to_string()))
            }
            _ => None,
        }
    }

    pub(super) fn lookup(app: Option<&str>, name: &str) -> Option<Value> {
        const BASE: &str = r"SOFTWARE\Policies\Mayorana";
        let own = app.map(|a| format!(r"{BASE}\{a}"));
        [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER]
            .into_iter()
            .find_map(|root| {
                own.as_deref()
                    .and_then(|key| get(root, key, name))
                    .or_else(|| get(root, BASE, name))
            })
    }
}

#[cfg(not(windows))]
mod registry {
    pub(super) fn lookup(_app: Option<&str>, _name: &str) -> Option<super::Value> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from(pairs: &[(&str, Value)]) -> Policy {
        let pairs = pairs.to_vec();
        read(&move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        })
    }

    #[test]
    fn nothing_set_changes_nothing() {
        assert_eq!(from(&[]), Policy::default());
        assert!(Policy::default().managed().is_empty());
    }

    #[test]
    fn what_is_managed_is_said_once() {
        let p = Policy {
            disable_update_check: true,
            disable_self_update: true,
            disable_telemetry: true,
            licence: Some("x".into()),
        };
        assert_eq!(p.managed().len(), 3);
    }

    #[test]
    fn a_switch_is_on_when_non_zero() {
        let p = from(&[
            ("DisableUpdateCheck", Value::Number(1)),
            ("DisableSelfUpdate", Value::Number(0)),
            ("DisableTelemetry", Value::Number(2)),
        ]);
        assert!(p.disable_update_check);
        assert!(!p.disable_self_update);
        assert!(p.disable_telemetry);
    }

    #[test]
    fn a_switch_written_as_text_is_ignored() {
        assert!(!from(&[("DisableTelemetry", Value::Text("1".into()))]).disable_telemetry);
    }

    #[test]
    fn the_licence_is_read_like_one_typed_in() {
        let typed = "  abcd-efgh ";
        let p = from(&[("Licence", Value::Text(typed.into()))]);
        assert_eq!(p.licence, crate::licence::normalise(typed));
        assert_eq!(
            from(&[("Licence", Value::Text("   ".into()))]).licence,
            None
        );
    }
}
