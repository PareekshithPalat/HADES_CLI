pub mod bus;
pub mod event;

pub use bus::EventBus;
pub use event::HadesEvent;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        let event = HadesEvent::app_started("0.1.0");
        let delivered = bus.publish(event.clone());
        assert_eq!(delivered, 2);

        let rec1 = rx1.recv().await.expect("rx1 receive");
        let rec2 = rx2.recv().await.expect("rx2 receive");

        assert_eq!(rec1, event);
        assert_eq!(rec2, event);
    }

    #[tokio::test]
    async fn test_publish_no_subscribers() {
        let bus = EventBus::new();
        let event = HadesEvent::error_occurred("test error");
        let delivered = bus.publish(event);
        assert_eq!(delivered, 0);
    }

    #[test]
    fn test_event_constructors() {
        let e1 = HadesEvent::app_started("0.1.0");
        assert!(matches!(e1, HadesEvent::ApplicationStarted { .. }));

        let e2 = HadesEvent::app_shutdown(Some("user exit".into()));
        assert!(matches!(e2, HadesEvent::ApplicationShutdown { .. }));

        let e3 = HadesEvent::command_entered("/status");
        assert!(matches!(e3, HadesEvent::CommandEntered { .. }));

        let e4 = HadesEvent::command_executed("/status", true);
        assert!(matches!(e4, HadesEvent::CommandExecuted { .. }));

        let e5 = HadesEvent::config_loaded(PathBuf::from("/tmp/config.toml"));
        assert!(matches!(e5, HadesEvent::ConfigLoaded { .. }));

        let e6 = HadesEvent::config_saved(PathBuf::from("/tmp/config.toml"));
        assert!(matches!(e6, HadesEvent::ConfigSaved { .. }));

        let e7 = HadesEvent::error_occurred("error");
        assert!(matches!(e7, HadesEvent::ErrorOccurred { .. }));
    }
}
