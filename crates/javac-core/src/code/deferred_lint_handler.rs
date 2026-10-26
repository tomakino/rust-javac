// Origin: com/sun/tools/javac/code/DeferredLintHandler.java @ 982d53c (jdk-17.0.19-ga)
//
// PORTING_RULES adapters (registered):
// - DiagnosticPosition object identity -> Rc<dyn DiagnosticPosition> compared
// with Rc::ptr_eq (the Java HashMap uses the default identity hashCode/equals);
// - the Java anonymous IMMEDIATE_POSITION DiagnosticPosition -> CurrentPos::Immediate
// (there is no object identity for the anonymous singleton in Rust; the state is
// modeled explicitly);
// - LintLogger -> Rust trait object; report() takes the logger by value (Java
// passes an interface instance; the ported callers pass a closure).
use std::rc::Rc;

use crate::ids::TreeId;
use crate::tree::end_pos_table::EndPosTable;
use crate::util::jc_diagnostic::DiagnosticPosition;

/// Java: public interface DeferredLintHandler.LintLogger
pub trait LintLogger {
    /// Java: void report()
    fn report(&mut self);
}

impl<F: FnMut()> LintLogger for F {
    fn report(&mut self) {
        self();
    }
}

/// The current position state: either the immediate position (the Java
/// IMMEDIATE_POSITION sentinel) or a real diagnostic position.
#[derive(Clone)]
pub enum CurrentPos {
    /// Java: IMMEDIATE_POSITION
    Immediate,
    /// A real DiagnosticPosition.
    Pos(Rc<dyn DiagnosticPosition>),
}

/// Java: the anonymous IMMEDIATE_POSITION DiagnosticPosition: every method is an
/// AssertionError upstream; the port models it as the Immediate state instead.
pub struct ImmediatePosition;

impl DiagnosticPosition for ImmediatePosition {
    fn get_tree(&self) -> Option<TreeId> {
        panic!("AssertionError")
    }
    fn get_start_position(&self) -> i32 {
        panic!("AssertionError")
    }
    fn get_preferred_position(&self) -> i32 {
        panic!("AssertionError")
    }
    fn get_end_position(&self, _end_pos_table: Option<&dyn EndPosTable>) -> i32 {
        panic!("AssertionError")
    }
}

/// Java: public class DeferredLintHandler
pub struct DeferredLintHandler {
    /// Java: private DiagnosticPosition currentPos
    current_pos: CurrentPos,
    /// Java: private Map<DiagnosticPosition, ListBuffer<LintLogger>> loggersQueue
    #[allow(clippy::type_complexity)]
    loggers_queue: Vec<(Rc<dyn DiagnosticPosition>, Vec<Box<dyn LintLogger>>)>,
}

impl DeferredLintHandler {
    /// Java: protected DeferredLintHandler(Context context)
    pub fn new() -> DeferredLintHandler {
        DeferredLintHandler {
            current_pos: CurrentPos::Immediate,
            loggers_queue: Vec::new(),
        }
    }

    /// Java: public void report(LintLogger logger)
    pub fn report(&mut self, mut logger: Box<dyn LintLogger>) {
        match &self.current_pos {
            CurrentPos::Immediate => logger.report(),
            CurrentPos::Pos(current) => {
                let current = Rc::clone(current);
                if let Some((_, loggers)) = self
                    .loggers_queue
                    .iter_mut()
                    .find(|(pos, _)| Rc::ptr_eq(pos, &current))
                {
                    loggers.push(logger);
                } else {
                    self.loggers_queue.push((current, vec![logger]));
                }
            }
        }
    }

    /// Java: public void flush(DiagnosticPosition pos)
    pub fn flush(&mut self, pos: &Rc<dyn DiagnosticPosition>) {
        if let Some(index) = self
            .loggers_queue
            .iter()
            .position(|(candidate, _)| Rc::ptr_eq(candidate, pos))
        {
            let (_, mut loggers) = self.loggers_queue.remove(index);
            for logger in loggers.iter_mut() {
                logger.report();
            }
        }
    }

    /// Java: public DiagnosticPosition setPos(DiagnosticPosition currentPos)
    pub fn set_pos(&mut self, current_pos: Rc<dyn DiagnosticPosition>) -> CurrentPos {
        std::mem::replace(&mut self.current_pos, CurrentPos::Pos(current_pos))
    }

    /// Restore a previously saved state (the Java callers pass the previous
    /// DiagnosticPosition back to setPos; the port models the IMMEDIATE state
    /// explicitly, hence this companion of set_pos).
    pub fn set_pos_state(&mut self, state: CurrentPos) -> CurrentPos {
        std::mem::replace(&mut self.current_pos, state)
    }

    /// Java: public DiagnosticPosition immediate()
    pub fn immediate(&mut self) -> CurrentPos {
        std::mem::replace(&mut self.current_pos, CurrentPos::Immediate)
    }
}

impl Default for DeferredLintHandler {
    fn default() -> Self {
        DeferredLintHandler::new()
    }
}
