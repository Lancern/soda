use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use thiserror::Error;

use crate::ctx::Context;

/// Represent a pass.
pub trait Pass: Any {
    /// Get the name of the pass.
    fn name(&self) -> &str;

    /// Run the pass.
    fn run(&mut self, ctx: &PassContext) -> Result<(), anyhow::Error>;
}

/// Provide context for running a single pass.
pub struct PassContext<'p, 's, 'd> {
    pub soda_ctx: &'s mut Context<'d>,
    passes: &'p [Box<dyn Pass>],
}

impl<'p, 's, 'd> PassContext<'p, 's, 'd> {
    /// Get the pass referenced by the given handle.
    ///
    /// # Panics
    ///
    /// This function will panic if either:
    /// - The given pass handle does not refer to a valid pass in the current context;
    /// - The pass referred to by the given pass handle does not have the specified type.
    pub fn get_pass<P>(&self, handle: PassHandle<P>) -> &P
    where
        P: Pass,
    {
        self.passes
            .get(handle.idx)
            .and_then(|boxed_pass| (&*boxed_pass as &dyn Any).downcast_ref::<P>())
            .unwrap()
    }
}

impl<'p, 's, 'd> Debug for PassContext<'p, 's, 'd> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let pass_names: Vec<_> = self.passes.iter().map(|pass| pass.name()).collect();
        f.debug_struct("PassContext")
            .field("soda_ctx", &self.soda_ctx)
            .field("passes", &pass_names)
            .finish()
    }
}

/// Manage and run a flow of passes.
pub struct PassManager {
    passes: Vec<Box<dyn Pass>>,
    current_pass_idx: usize,
}

impl PassManager {
    /// Create a new `PassManager` that does not contain any passes.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            current_pass_idx: 0,
        }
    }

    /// Add a pass to the end of the current pass pipeline.
    pub fn add_pass<P>(&mut self, pass: P) -> PassHandle<P>
    where
        P: Pass,
    {
        let idx = self.passes.len();
        self.passes.push(Box::new(pass));
        PassHandle::new(idx)
    }

    /// Add a pass to the end of the current pass pipeline.
    pub fn add_pass_default<P>(&mut self) -> PassHandle<P>
    where
        P: Pass + Default,
    {
        self.add_pass(P::default())
    }

    /// Run the pass pipeline.
    pub fn run(mut self, ctx: &mut Context) -> Result<(), RunPassError> {
        while self.current_pass_idx < self.passes.len() {
            let (finished_passes, coming_passes) = self.passes.split_at_mut(self.current_pass_idx);
            let current_pass = &mut *coming_passes[0];

            let ctx = PassContext {
                soda_ctx: ctx,
                passes: finished_passes,
            };
            if let Err(err) = current_pass.run(&ctx) {
                return Err(RunPassError {
                    name: String::from(current_pass.name()),
                    error: err,
                });
            }

            self.current_pass_idx += 1;
        }

        Ok(())
    }
}

impl Debug for PassManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let pass_names: Vec<_> = self.passes.iter().map(|p| p.name()).collect();
        f.debug_struct("PassManager")
            .field("passes", &pass_names)
            .field("current_pass_idx", &self.current_pass_idx)
            .finish()
    }
}

/// A lightweight handle to a pass in a [`PassManager`].
pub struct PassHandle<P> {
    idx: usize,
    _phantom: PhantomData<*const P>,
}

impl<P> PassHandle<P> {
    fn new(idx: usize) -> Self {
        Self {
            idx,
            _phantom: PhantomData::default(),
        }
    }
}

impl<P> Clone for PassHandle<P> {
    fn clone(&self) -> Self {
        Self {
            idx: self.idx.clone(),
            _phantom: PhantomData::default(),
        }
    }
}

impl<P> Copy for PassHandle<P> {}

impl<P> Debug for PassHandle<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassHandle")
            .field("idx", &self.idx)
            .finish()
    }
}

/// Errors occured when running a pass pipeline.
#[derive(Debug, Error)]
#[error("pass {name} failed: {error:?}")]
pub struct RunPassError {
    /// The name of the specific pass that failed.
    pub name: String,

    /// The error value produced by the failed pass.
    #[source]
    pub error: anyhow::Error,
}
