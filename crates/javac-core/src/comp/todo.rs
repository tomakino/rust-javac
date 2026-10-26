// Source: com/sun/tools/javac/comp/Todo.java

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
    pub file_contents: Vec<Rc<Env<AttrContext>>>,
    /// The source file of the queue.
    pub file: Option<Rc<dyn JavaFileObject>>,
}

impl FileQueue {
    /// Create an empty file queue for the given file.
    pub fn new(file: Option<Rc<dyn JavaFileObject>>) -> FileQueue {
        FileQueue {
            file_contents: Vec::new(),
            file,
        }
    }

    /// Return whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.file_contents.is_empty()
    }

    /// Return the number of environments in the queue.
    pub fn size(&self) -> i32 {
        self.file_contents.len() as i32
    }
}

/// A queue of all as yet unattributed classes.
pub struct Todo {
    /// The environments, in insertion order.
    pub contents: Vec<Rc<Env<AttrContext>>>,
    /// The per-file queues, when grouping has been requested.
    pub contents_by_file: Option<Vec<Rc<RefCell<FileQueue>>>>,
    /// The per-file lookup table.
    pub file_map: Vec<(Rc<dyn JavaFileObject>, Rc<RefCell<FileQueue>>)>,
}

impl Todo {
    /// Create an empty todo list.
    ///
    /// # Arguments
    ///   - context: the compiler context service table.
    ///
    /// # Returns
    ///   - the todo list registered in the context.
    ///
    /// # Panics
    ///   - none.
    pub fn instance(context: &mut Context) -> Rc<Todo> {
        if let Some(instance) = context.get::<Todo>() {
            return instance;
        }
        let todo = Rc::new(Todo::new_empty());
        context.put(Rc::clone(&todo));
        todo
    }

    /// Create an empty todo list.
    fn new_empty() -> Todo {
        Todo {
            contents: Vec::new(),
            contents_by_file: None,
            file_map: Vec::new(),
        }
    }

    /// Append an environment to the end of the todo list.
    pub fn append(&mut self, ctx: &CompilerContext, env: Rc<Env<AttrContext>>) {
        self.offer(ctx, env);
    }

    /// Return the number of environments in the todo list.
    pub fn size(&self) -> i32 {
        self.contents.len() as i32
    }

    /// Add an environment to the end of the todo list.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - e: the environment to add.
    ///
    /// # Returns
    ///   - true when the environment was not already present.
    ///
    /// # Panics
    ///   - none.
    pub fn offer(&mut self, ctx: &CompilerContext, e: Rc<Env<AttrContext>>) -> bool {
        if self.contents.iter().any(|env| Rc::ptr_eq(env, &e)) {
            return false;
        }
        self.contents.push(Rc::clone(&e));
        if self.contents_by_file.is_some() {
            self.add_by_file(ctx, &e);
        }
        true
    }

    /// Remove all environments except those belonging to the given files.
    pub fn retain_files(
        &mut self,
        ctx: &CompilerContext,
        source_files: &[Option<Rc<dyn JavaFileObject>>],
    ) {
        let mut kept: Vec<Rc<Env<AttrContext>>> = Vec::new();
        for env in std::mem::take(&mut self.contents) {
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
            } else if self.contents_by_file.is_some() {
                self.remove_by_file(ctx, &env);
            }
        }
        self.contents = kept;
    }

    /// Remove and return the first environment of the todo list.
    pub fn poll(&mut self, ctx: &CompilerContext) -> Option<Rc<Env<AttrContext>>> {
        if self.contents.is_empty() {
            return None;
        }
        let env = self.contents.remove(0);
        if self.contents_by_file.is_some() {
            self.remove_by_file(ctx, &env);
        }
        Some(env)
    }

    /// Return the first environment of the todo list without removing it.
    pub fn peek(&self) -> Option<Rc<Env<AttrContext>>> {
        self.contents.first().cloned()
    }

    /// Return the per-file queues, building them when needed.
    pub fn group_by_file(&mut self, ctx: &CompilerContext) -> Vec<Rc<RefCell<FileQueue>>> {
        if self.contents_by_file.is_none() {
            self.contents_by_file = Some(Vec::new());
            let contents = self.contents.clone();
            for env in contents {
                self.add_by_file(ctx, &env);
            }
        }
        self.contents_by_file.clone().expect("per-file queues")
    }

    /// Add an environment to the per-file queue of its source file.
    fn add_by_file(&mut self, ctx: &CompilerContext, env: &Rc<Env<AttrContext>>) {
        let file = env_source_file(ctx, env);
        let existing = match &file {
            Some(file) => self
                .file_map
                .iter()
                .find(|(candidate, _)| Rc::ptr_eq(candidate, file))
                .map(|(_, queue)| Rc::clone(queue)),
            None => None,
        };
        let queue = match existing {
            Some(queue) => queue,
            None => {
                let queue = Rc::new(RefCell::new(FileQueue::new(file.clone())));
                if let Some(file) = file {
                    self.file_map.push((file, Rc::clone(&queue)));
                }
                self.contents_by_file
                    .as_mut()
                    .expect("per-file queues")
                    .push(Rc::clone(&queue));
                queue
            }
        };
        queue.borrow_mut().file_contents.push(Rc::clone(env));
    }

    /// Remove an environment from the per-file queue of its source file.
    fn remove_by_file(&mut self, ctx: &CompilerContext, env: &Rc<Env<AttrContext>>) {
        let file = env_source_file(ctx, env);
        let Some(file) = file else {
            return;
        };
        let found = self
            .file_map
            .iter()
            .find(|(candidate, _)| Rc::ptr_eq(candidate, &file))
            .map(|(_, queue)| Rc::clone(queue));
        let Some(queue) = found else {
            return;
        };
        let mut queue_ref = queue.borrow_mut();
        let before = queue_ref.file_contents.len();
        queue_ref
            .file_contents
            .retain(|candidate| !Rc::ptr_eq(candidate, env));
        let removed = queue_ref.file_contents.len() != before;
        drop(queue_ref);
        if removed && queue.borrow().is_empty() {
            self.file_map
                .retain(|(candidate, _)| !Rc::ptr_eq(candidate, &file));
            if let Some(queues) = &mut self.contents_by_file {
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
