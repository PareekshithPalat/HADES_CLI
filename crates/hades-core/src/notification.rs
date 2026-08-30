use std::io::{stdout, Write};
use std::process::Command;
use tracing::{debug, warn};

use hades_config::NotificationConfig;
use hades_events::{EventBus, HadesEvent};

/// Categorizes different operational notification types requiring distinct sounds and alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// CLI requires interactive user input, permission approval, or prompt input.
    InputRequired,
    /// Task, command, or AI generation finished execution successfully.
    TaskCompleted,
    /// An error or verification failure occurred.
    Error,
}

impl NotificationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InputRequired => "input_required",
            Self::TaskCompleted => "task_completed",
            Self::Error => "error",
        }
    }
}

/// Plays distinct non-blocking audio sound effects for CLI events.
pub struct SoundPlayer;

impl SoundPlayer {
    /// Plays an audio chime matching the specified notification kind.
    pub fn play(kind: NotificationKind, sound_theme: &str) {
        if sound_theme == "bell_only" {
            Self::play_terminal_bell(kind);
            return;
        }

        #[cfg(target_os = "macos")]
        {
            let sound_file = match kind {
                NotificationKind::InputRequired => "/System/Library/Sounds/Glass.aiff",
                NotificationKind::TaskCompleted => "/System/Library/Sounds/Ping.aiff",
                NotificationKind::Error => "/System/Library/Sounds/Basso.aiff",
            };

            let played = Command::new("afplay")
                .arg(sound_file)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok();

            if !played {
                Self::play_terminal_bell(kind);
            }
        }

        #[cfg(target_os = "linux")]
        {
            let sound_file = match kind {
                NotificationKind::InputRequired => {
                    "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga"
                }
                NotificationKind::TaskCompleted => {
                    "/usr/share/sounds/freedesktop/stereo/complete.oga"
                }
                NotificationKind::Error => "/usr/share/sounds/freedesktop/stereo/dialog-error.oga",
            };

            let played = Command::new("paplay")
                .arg(sound_file)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .or_else(|_| {
                    Command::new("aplay")
                        .arg(sound_file)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                })
                .is_ok();

            if !played {
                Self::play_terminal_bell(kind);
            }
        }

        #[cfg(target_os = "windows")]
        {
            let (freq1, freq2) = match kind {
                NotificationKind::InputRequired => (880, 1046),
                NotificationKind::TaskCompleted => (523, 784),
                NotificationKind::Error => (330, 220),
            };

            let cmd = format!("[Console]::Beep({}, 120); [Console]::Beep({}, 180)", freq1, freq2);
            let played = Command::new("powershell")
                .args(["-NoProfile", "-Command", &cmd])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok();

            if !played {
                Self::play_terminal_bell(kind);
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Self::play_terminal_bell(kind);
        }
    }

    /// Emits ASCII terminal bell characters (`\x07`) to standard output.
    pub fn play_terminal_bell(kind: NotificationKind) {
        let count = match kind {
            NotificationKind::InputRequired => 2,
            NotificationKind::TaskCompleted => 1,
            NotificationKind::Error => 3,
        };

        let mut out = stdout();
        for _ in 0..count {
            let _ = print!("\x07");
        }
        let _ = out.flush();
    }
}

/// Central notification service coordinating audio play and desktop OS pop-up alerts.
#[derive(Clone)]
pub struct NotificationService {
    config: NotificationConfig,
    event_bus: Option<EventBus>,
}

impl NotificationService {
    /// Creates a new `NotificationService` with default config.
    pub fn new(config: NotificationConfig, event_bus: Option<EventBus>) -> Self {
        Self { config, event_bus }
    }

    /// Updates active notification configuration.
    pub fn update_config(&mut self, config: NotificationConfig) {
        self.config = config;
    }

    /// Returns a reference to current notification settings.
    pub fn config(&self) -> &NotificationConfig {
        &self.config
    }

    /// Dispatches notification alerts asynchronously based on kind and configuration rules.
    pub fn notify(&self, kind: NotificationKind, title: &str, message: &str) {
        if !self.config.enabled {
            return;
        }

        // Check event-specific filtering
        let should_notify = match kind {
            NotificationKind::InputRequired => self.config.notify_on_input_required,
            NotificationKind::TaskCompleted => self.config.notify_on_task_completed,
            NotificationKind::Error => self.config.notify_on_error,
        };

        if !should_notify {
            return;
        }

        let sound_enabled = self.config.sound_enabled;
        let desktop_enabled = self.config.desktop_enabled;
        let sound_theme = self.config.sound_theme.clone();
        let title_owned = title.to_string();
        let message_owned = message.to_string();

        debug!(
            kind = ?kind,
            sound = sound_enabled,
            desktop = desktop_enabled,
            "Triggering notification"
        );

        // Spawn background task for non-blocking playback and desktop alert dispatch
        std::thread::spawn(move || {
            let mut sound_played = false;
            let mut desktop_sent = false;

            if sound_enabled {
                SoundPlayer::play(kind, &sound_theme);
                sound_played = true;
            }

            if desktop_enabled {
                let summary = format!("Hades CLI - {}", title_owned);
                let result = notify_rust::Notification::new()
                    .summary(&summary)
                    .body(&message_owned)
                    .appname("Hades CLI")
                    .show();

                if let Err(e) = result {
                    warn!(error = %e, "Desktop notification dispatch failed");
                } else {
                    desktop_sent = true;
                }
            }

            let _ = (sound_played, desktop_sent);
        });

        if let Some(ref bus) = self.event_bus {
            bus.publish(HadesEvent::NotificationTriggered {
                timestamp: chrono::Utc::now(),
                kind: kind.as_str().to_string(),
                sound_played: self.config.sound_enabled,
                desktop_sent: self.config.desktop_enabled,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_kind_as_str() {
        assert_eq!(NotificationKind::InputRequired.as_str(), "input_required");
        assert_eq!(NotificationKind::TaskCompleted.as_str(), "task_completed");
        assert_eq!(NotificationKind::Error.as_str(), "error");
    }

    #[test]
    fn test_notification_service_disabled() {
        let mut config = NotificationConfig::default();
        config.enabled = false;

        let service = NotificationService::new(config, None);
        // Should return early without crashing
        service.notify(NotificationKind::InputRequired, "Test", "Test message");
    }
}
