use std::io::{stdout, Write};
use std::process::Command;
use tracing::debug;

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

/// Plays distinct, attention-demanding non-blocking audio sound effects for CLI events.
pub struct SoundPlayer;

impl SoundPlayer {
    /// Plays an urgent audio chime and terminal bell matching the specified notification kind.
    pub fn play(kind: NotificationKind, sound_theme: &str) {
        // Emit terminal bell characters immediately to trigger terminal visual/audio bell
        Self::play_terminal_bell(kind);

        if sound_theme == "bell_only" {
            return;
        }

        #[cfg(target_os = "macos")]
        {
            let (first_sound, second_sound) = match kind {
                NotificationKind::InputRequired => (
                    "/System/Library/Sounds/Sosumi.aiff",
                    Some("/System/Library/Sounds/Glass.aiff"),
                ),
                NotificationKind::TaskCompleted => (
                    "/System/Library/Sounds/Hero.aiff",
                    Some("/System/Library/Sounds/Ping.aiff"),
                ),
                NotificationKind::Error => (
                    "/System/Library/Sounds/Basso.aiff",
                    Some("/System/Library/Sounds/Sosumi.aiff"),
                ),
            };

            let _ = Command::new("afplay").arg(first_sound).status();

            if let Some(second) = second_sound {
                let _ = Command::new("afplay").arg(second).status();
            }
        }

        #[cfg(target_os = "linux")]
        {
            let sound_file = match kind {
                NotificationKind::InputRequired => {
                    "/usr/share/sounds/freedesktop/stereo/dialog-warning.oga"
                }
                NotificationKind::TaskCompleted => {
                    "/usr/share/sounds/freedesktop/stereo/complete.oga"
                }
                NotificationKind::Error => "/usr/share/sounds/freedesktop/stereo/dialog-error.oga",
            };

            let _ = Command::new("paplay")
                .arg(sound_file)
                .status()
                .or_else(|_| Command::new("aplay").arg(sound_file).status());
        }

        #[cfg(target_os = "windows")]
        {
            let cmd = match kind {
                NotificationKind::InputRequired => {
                    "[Console]::Beep(1200, 120); [Console]::Beep(1800, 150); [Console]::Beep(1200, 120); [Console]::Beep(1800, 200)"
                }
                NotificationKind::TaskCompleted => {
                    "[Console]::Beep(523, 120); [Console]::Beep(659, 120); [Console]::Beep(784, 180)"
                }
                NotificationKind::Error => {
                    "[Console]::Beep(400, 200); [Console]::Beep(300, 300)"
                }
            };

            let _ = Command::new("powershell")
                .args(["-NoProfile", "-Command", cmd])
                .status();
        }
    }

    /// Emits ASCII terminal bell characters (`\x07`) to standard output.
    pub fn play_terminal_bell(kind: NotificationKind) {
        let count = match kind {
            NotificationKind::InputRequired => 4,
            NotificationKind::TaskCompleted => 2,
            NotificationKind::Error => 4,
        };

        let mut out = stdout();
        for _ in 0..count {
            print!("\x07");
        }
        let _ = out.flush();
    }
}

/// Central notification service coordinating in-terminal audio sound alerts.
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
        let sound_theme = self.config.sound_theme.clone();
        let _title_owned = title.to_string();
        let _message_owned = message.to_string();

        debug!(
            kind = ?kind,
            sound = sound_enabled,
            "Triggering in-terminal notification alert"
        );

        // Spawn background thread for non-blocking sound playback
        std::thread::spawn(move || {
            if sound_enabled {
                SoundPlayer::play(kind, &sound_theme);
            }
        });

        if let Some(ref bus) = self.event_bus {
            bus.publish(HadesEvent::NotificationTriggered {
                timestamp: chrono::Utc::now(),
                kind: kind.as_str().to_string(),
                sound_played: self.config.sound_enabled,
                desktop_sent: false,
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
        let config = NotificationConfig {
            enabled: false,
            ..Default::default()
        };

        let service = NotificationService::new(config, None);
        // Should return early without crashing
        service.notify(NotificationKind::InputRequired, "Test", "Test message");
    }
}
