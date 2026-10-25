// Origin: com/sun/source/util/TaskListener.java @ 982d53c (jdk-17.0.19-ga)

//! Provides a listener to monitor the activity of the JDK Java Compiler, javac.
//!
//! PORTING_RULES: Java interface -> Rust trait; the two Java `default` methods
//! do nothing and are Rust default methods.

use super::task_event::TaskEvent;

/// Provides a listener to monitor the activity of the JDK Java Compiler, javac.
///
/// @author Jonathan Gibbons
/// @since 1.6
pub trait TaskListener {
    /// Invoked when an event has begun.
    ///
    /// @implSpec The default implementation of this method does nothing.
    ///
    /// @param e the event
    fn started(&mut self, e: &TaskEvent) {
        let _ = e;
    }

    /// Invoked when an event has been completed.
    ///
    /// @implSpec The default implementation of this method does nothing.
    ///
    /// @param e the event
    fn finished(&mut self, e: &TaskEvent) {
        let _ = e;
    }
}

#[cfg(test)]
mod tests {
    use super::TaskListener;
    use crate::util::task_event::{Kind, TaskEvent};

    struct Recorder {
        started: usize,
        finished: usize,
    }

    impl TaskListener for Recorder {
        fn started(&mut self, _e: &TaskEvent) {
            self.started += 1;
        }
        fn finished(&mut self, _e: &TaskEvent) {
            self.finished += 1;
        }
    }

    #[test]
    fn listener_is_object_safe_with_default_noops() {
        let event = TaskEvent::new(Kind::PARSE);
        // the Java default methods do nothing
        struct Empty;
        impl TaskListener for Empty {}
        let mut empty = Empty;
        let listener: &mut dyn TaskListener = &mut empty;
        listener.started(&event);
        listener.finished(&event);

        let mut recorder = Recorder { started: 0, finished: 0 };
        let listener: &mut dyn TaskListener = &mut recorder;
        listener.started(&event);
        listener.finished(&event);
        assert_eq!((recorder.started, recorder.finished), (1, 1));
    }
}
