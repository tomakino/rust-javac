//! A queue of all as yet unattributed classes.

use std::cell::RefCell;
use std::rc::Rc;

use javac_tools::tools::java_file_object::JavaFileObject;

use crate::comp::attr_context::AttrContext;
use crate::comp::env::Env;
use crate::compiler_context::CompilerContext;
use crate::util::context::Context;

/// One queue of the environments belonging to the same source file.
pub struct FileQueue {
    /// The environments of this file.
    pub file_contents: RefCell<Vec<Rc<Env<AttrContext>>>>,
    /// The source file of the queue.
    pub file: Option<Rc<dyn JavaFileObject>>,
}

impl FileQueue {
    pub fn new(file: Option<Rc<dyn JavaFileObject>>) -> FileQueue {
        FileQueue {
            file_contents: RefCell::new(Vec::new()),
            file,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.file_contents.borrow().is_empty()
    }

    pub fn size(&self) -> i32 {
        self.file_contents.borrow().len() as i32
    }
}

/// A queue of all as yet unattributed classes.
pub struct Todo {
    /// The environments, in insertion order.
    contents: RefCell<Vec<Rc<Env<AttrContext>>>>,
    /// The per-file queues, when grouping has been requested.
    contents_by_file: RefCell<Option<Vec<FileQueueRef>>>,
    /// The per-file lookup table.
    file_map: RefCell<Vec<(Rc<dyn JavaFileObject>, FileQueueRef)>>,
}

/// One shared per-file queue.
pub type FileQueueRef = Rc<FileQueue>;

/// The shared todo list (Java: the Todo singleton).
pub type TodoRef = Rc<Todo>;

impl Todo {
    /// Create or fetch the todo list registered in the context.
    pub fn instance(context: &mut Context) -> TodoRef {
        if let Some(instance) = context.get::<Todo>() {
            return instance;
        }
        let todo = Rc::new(Todo::new_empty());
        context.put(Rc::clone(&todo));
        todo
    }

    fn new_empty() -> Todo {
        Todo {
            contents: RefCell::new(Vec::new()),
            contents_by_file: RefCell::new(None),
            file_map: RefCell::new(Vec::new()),
        }
    }

    /// Append an environment to the end of the todo list.
    pub fn append(&self, ctx: &CompilerContext, env: Rc<Env<AttrContext>>) {
        self.offer(ctx, env);
    }

    /// Return the number of environments in the todo list.
    pub fn size(&self) -> i32 {
        self.contents.borrow().len() as i32
    }

    /// Add an environment to the end of the todo list.
    pub fn offer(&self, ctx: &CompilerContext, e: Rc<Env<AttrContext>>) -> bool {
        {
            let mut contents = self.contents.borrow_mut();
            if contents.iter().any(|env| Rc::ptr_eq(env, &e)) {
                return false;
            }
            contents.push(Rc::clone(&e));
        }
        if self.contents_by_file.borrow().is_some() {
            self.add_by_file(ctx, &e);
        }
        true
    }

    /// Remove all environments except those belonging to the given files.
    pub fn retain_files(
        &self,
        ctx: &CompilerContext,
        source_files: &[Option<Rc<dyn JavaFileObject>>],
    ) {
        let mut kept: Vec<Rc<Env<AttrContext>>> = Vec::new();
        let mut contents = self.contents.borrow_mut();
        for env in std::mem::take(&mut *contents) {
            let source = env_source_file(ctx, &env);
            let keep = match &source {
                Some(source) => source_files
                    .iter()
                    .flatten()
                    .any(|candidate| Rc::ptr_eq(candidate, source)),
                None => false,
            };
            if keep {
                kept.push(env);
            } else if self.contents_by_file.borrow().is_some() {
                self.remove_by_file(ctx, &env);
            }
        }
        *contents = kept;
    }

    /// Remove and return the first environment of the todo list.
    pub fn poll(&self, ctx: &CompilerContext) -> Option<Rc<Env<AttrContext>>> {
        if self.contents.borrow().is_empty() {
            return None;
        }
        let env = self.contents.borrow_mut().remove(0);
        if self.contents_by_file.borrow().is_some() {
            self.remove_by_file(ctx, &env);
        }
        Some(env)
    }

    /// Return the first environment of the todo list without removing it.
    pub fn peek(&self) -> Option<Rc<Env<AttrContext>>> {
        self.contents.borrow().first().cloned()
    }

    /// Return the per-file queues, building them when needed.
    pub fn group_by_file(&self, ctx: &CompilerContext) -> Vec<FileQueueRef> {
        if self.contents_by_file.borrow().is_none() {
            *self.contents_by_file.borrow_mut() = Some(Vec::new());
            let contents = self.contents.borrow().clone();
            for env in contents {
                self.add_by_file(ctx, &env);
            }
        }
        self.contents_by_file
            .borrow()
            .clone()
            .expect("per-file queues")
    }

    /// Add an environment to the per-file queue of its source file.
    fn add_by_file(&self, ctx: &CompilerContext, env: &Rc<Env<AttrContext>>) {
        let file = env_source_file(ctx, env);
        let existing = match &file {
            Some(file) => self
                .file_map
                .borrow()
                .iter()
                .find(|(candidate, _)| Rc::ptr_eq(candidate, file))
                .map(|(_, queue)| Rc::clone(queue)),
            None => None,
        };
        let queue = match existing {
            Some(queue) => queue,
            None => {
                let queue = Rc::new(FileQueue::new(file.clone()));
                if let Some(file) = file {
                    self.file_map
                        .borrow_mut()
                        .push((file, Rc::clone(&queue)));
                }
                self.contents_by_file
                    .borrow_mut()
                    .as_mut()
                    .expect("per-file queues")
                    .push(Rc::clone(&queue));
                queue
            }
        };
        queue.file_contents.borrow_mut().push(Rc::clone(env));
    }

    /// Remove an environment from the per-file queue of its source file.
    fn remove_by_file(&self, ctx: &CompilerContext, env: &Rc<Env<AttrContext>>) {
        let file = env_source_file(ctx, env);
        let Some(file) = file else {
            return;
        };
        let found = self
            .file_map
            .borrow()
            .iter()
            .find(|(candidate, _)| Rc::ptr_eq(candidate, &file))
            .map(|(_, queue)| Rc::clone(queue));
        let Some(queue) = found else {
            return;
        };
        let mut contents = queue.file_contents.borrow_mut();
        let before = contents.len();
        contents.retain(|candidate| !Rc::ptr_eq(candidate, env));
        let removed = contents.len() != before;
        drop(contents);
        if removed && queue.is_empty() {
            self.file_map
                .borrow_mut()
                .retain(|(candidate, _)| !Rc::ptr_eq(candidate, &file));
            if let Some(queues) = self.contents_by_file.borrow_mut().as_mut() {
                queues.retain(|candidate| !Rc::ptr_eq(candidate, &queue));
            }
        }
    }
}

/// Return the source file of the top level tree of an environment.
fn env_source_file(
    ctx: &CompilerContext,
    env: &Rc<Env<AttrContext>>,
) -> Option<Rc<dyn JavaFileObject>> {
    let toplevel = env.toplevel?;
    match &ctx.tree(toplevel).kind {
        crate::tree::jc_tree::TreeKind::CompilationUnit { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}
