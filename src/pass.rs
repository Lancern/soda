use std::alloc::Layout;
use std::any::TypeId;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use thiserror::Error;

use crate::ctx::Context;

/// Represent a pass.
pub trait Pass: 'static {
    const NAME: &'static str;

    type Output<'a>;
    type Error: Error + Send + Sync;

    /// Run the pass.
    fn run<'d>(
        &mut self,
        ctx: &PassContext<'_, 'd>,
        soda: &mut Context<'d>,
    ) -> Result<Self::Output<'d>, Self::Error>;
}

/// Provide context for running a single pass.
pub struct PassContext<'p, 'd> {
    pass_outputs: &'p [PassOutputSlot<'d>],
}

impl<'p, 'd> PassContext<'p, 'd> {
    /// Get the value produced by the pass referenced by the given handle.
    ///
    /// # Panics
    ///
    /// This function will panic if either:
    /// - The given pass handle does not refer to a valid pass in the current context;
    /// - The pass referred to by the given pass handle does not have the specified type.
    pub fn get_pass_output<P: Pass>(&self, handle: PassHandle<P>) -> &P::Output<'d> {
        self.pass_outputs
            .get(handle.idx)
            .map(|output| output.get::<P>())
            .unwrap()
    }
}

impl<'p, 'd> Debug for PassContext<'p, 'd> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassContext")
            .field(
                "pass_outputs",
                &format!("[{} values]", self.pass_outputs.len()),
            )
            .finish()
    }
}

/// Manage and run a flow of passes.
pub struct PassManager<'d> {
    passes: Vec<Box<dyn AbstractPass>>,
    pass_outputs: Vec<PassOutputSlot<'d>>,
    current_pass_idx: usize,
}

impl<'d> PassManager<'d> {
    /// Create a new `PassManager` that does not contain any passes.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            pass_outputs: Vec::new(),
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
    pub fn run(mut self, ctx: &mut Context<'d>) -> Result<(), RunPassError> {
        while self.current_pass_idx < self.passes.len() {
            let current_pass = &mut *self.passes[self.current_pass_idx];

            let pass_ctx = PassContext {
                pass_outputs: &self.pass_outputs,
            };
            match current_pass.run(&pass_ctx, ctx) {
                Ok(result) => {
                    self.pass_outputs.push(result);
                }
                Err(err) => {
                    return Err(RunPassError {
                        name: String::from(current_pass.name()),
                        error: err,
                    });
                }
            }

            self.current_pass_idx += 1;
        }

        Ok(())
    }
}

impl<'d> Debug for PassManager<'d> {
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

trait AbstractPass {
    fn name(&self) -> &'static str;

    fn run<'d>(
        &mut self,
        ctx: &PassContext<'_, 'd>,
        soda: &mut Context<'d>,
    ) -> anyhow::Result<PassOutputSlot<'d>>;
}

impl<P: Pass> AbstractPass for P {
    fn name(&self) -> &'static str {
        P::NAME
    }

    fn run<'d>(
        &mut self,
        ctx: &PassContext<'_, 'd>,
        soda: &mut Context<'d>,
    ) -> anyhow::Result<PassOutputSlot<'d>> {
        let output = <P as Pass>::run(self, ctx, soda)?;
        Ok(PassOutputSlot::new::<P>(output))
    }
}

struct PassOutputSlot<'d> {
    data_ptr: *mut u8,
    data_layout: Layout,
    pass_ty: TypeId,
    inplace_dropper: Box<dyn FnMut(*mut u8)>,
    _phantom: PhantomData<&'d ()>,
}

impl<'d> PassOutputSlot<'d> {
    fn new<P: Pass>(output: P::Output<'d>) -> Self {
        let data_layout = Layout::for_value(&output);
        let data_ptr = unsafe {
            let ptr = std::alloc::alloc(data_layout);
            assert!(!ptr.is_null());
            std::ptr::write(ptr as *mut P::Output<'d>, output);
            ptr
        };

        Self {
            data_ptr,
            data_layout,
            pass_ty: TypeId::of::<P>(),
            inplace_dropper: Box::new(|ptr| unsafe {
                std::ptr::drop_in_place(ptr as *mut P::Output<'d>)
            }),
            _phantom: PhantomData::default(),
        }
    }

    fn get<P: Pass>(&self) -> &P::Output<'d> {
        assert_eq!(self.pass_ty, TypeId::of::<P>());
        let value_ptr = self.data_ptr as *mut P::Output<'d>;
        unsafe { value_ptr.as_ref().unwrap() }
    }
}

impl<'d> Drop for PassOutputSlot<'d> {
    fn drop(&mut self) {
        (self.inplace_dropper)(self.data_ptr);
        unsafe {
            std::alloc::dealloc(self.data_ptr, self.data_layout);
        }
    }
}
