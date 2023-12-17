use std::any::Any;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use object::write::Object as OutputObject;
use thiserror::Error;

/// Represent a pass.
pub trait Pass<I> {
    const NAME: &'static str;

    type Output: 'static;
    type Error: Error + Send + Sync + 'static;

    /// Run the pass.
    fn run(&mut self, ctx: &PassContext<I>) -> Result<Self::Output, Self::Error>;
}

/// Provide context for running a single pass.
pub struct PassContext<I> {
    pub input: I,
    pub output: RefCell<OutputObject<'static>>,
    pass_outputs: Vec<Box<dyn Any>>,
}

impl<I> PassContext<I> {
    /// Get the value produced by the pass referenced by the given handle.
    ///
    /// # Panics
    ///
    /// This function will panic if either:
    /// - The given pass handle does not refer to a valid pass in the current context;
    /// - The pass referred to by the given pass handle does not have the specified type.
    pub fn get_pass_output<P>(&self, handle: PassHandle<P>) -> &P::Output
    where
        P: Pass<I>,
    {
        self.pass_outputs
            .get(handle.idx)
            .map(|output| output.downcast_ref().unwrap())
            .unwrap()
    }
}

impl<I> Debug for PassContext<I>
where
    I: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassContext")
            .field("input", &self.input)
            .field("output", &self.output)
            .field(
                "pass_outputs",
                &format!("[{} values]", self.pass_outputs.len()),
            )
            .finish()
    }
}

/// Manage and run a flow of passes.
#[derive(Default)]
pub struct PassManager<I> {
    passes: Vec<Box<dyn AbstractPass<I>>>,
}

impl<I> PassManager<I> {
    /// Create a new `PassManager` that does not contain any passes.
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    /// Add a pass to the end of the current pass pipeline.
    pub fn add_pass<P>(&mut self, pass: P) -> PassHandle<P>
    where
        P: Pass<I> + 'static,
    {
        let idx = self.passes.len();
        self.passes.push(Box::new(pass));
        PassHandle::new(idx)
    }

    /// Add a pass to the end of the current pass pipeline. The pass object is created via `Default::default`.
    pub fn add_pass_default<P>(&mut self) -> PassHandle<P>
    where
        P: Default + Pass<I> + 'static,
    {
        self.add_pass(P::default())
    }

    /// Run the pass pipeline.
    pub fn run(
        mut self,
        input: I,
        output: OutputObject<'static>,
    ) -> Result<OutputObject<'static>, RunPassError> {
        let mut ctx = PassContext {
            input,
            output: RefCell::new(output),
            pass_outputs: Vec::with_capacity(self.passes.len()),
        };

        for current_pass in &mut self.passes {
            log::info!("Running pass \"{}\" ...", current_pass.name());
            match current_pass.run(&ctx) {
                Ok(result) => {
                    ctx.pass_outputs.push(result);
                }
                Err(err) => {
                    return Err(RunPassError {
                        name: String::from(current_pass.name()),
                        error: err,
                    });
                }
            }
        }

        Ok(ctx.output.into_inner())
    }
}

impl<I> Debug for PassManager<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let pass_names: Vec<_> = self.passes.iter().map(|p| p.name()).collect();
        f.debug_struct("PassManager")
            .field("passes", &pass_names)
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

trait AbstractPass<I> {
    fn name(&self) -> &'static str;
    fn run(&mut self, ctx: &PassContext<I>) -> anyhow::Result<Box<dyn Any>>;
}

impl<I, P> AbstractPass<I> for P
where
    P: Pass<I>,
{
    fn name(&self) -> &'static str {
        P::NAME
    }

    fn run(&mut self, ctx: &PassContext<I>) -> anyhow::Result<Box<dyn Any>> {
        let output = <P as Pass<I>>::run(self, ctx)?;
        Ok(Box::new(output))
    }
}

#[cfg(test)]
pub mod test {
    use std::convert::Infallible;

    use super::*;

    pub trait PassTest: 'static {
        type Input;
        type Pass: Pass<Self::Input>;

        fn setup(&mut self, pass_mgr: &mut PassManager<Self::Input>) -> PassHandle<Self::Pass>;

        #[allow(unused_variables)]
        fn check_pass_output(&mut self, output: &<Self::Pass as Pass<Self::Input>>::Output) {}

        #[allow(unused_variables)]
        fn check_output_object(&mut self, output: &OutputObject<'static>) {}
    }

    pub fn run_pass_test<T>(mut test: T, input: T::Input, output: OutputObject<'static>)
    where
        T: PassTest,
    {
        let mut pass_mgr = PassManager::new();
        let target_pass = test.setup(&mut pass_mgr);

        pass_mgr.add_pass(TestPass { test, target_pass });
        pass_mgr.run(input, output).unwrap();
    }

    struct TestPass<T>
    where
        T: PassTest,
    {
        test: T,
        target_pass: PassHandle<T::Pass>,
    }

    impl<T> Pass<T::Input> for TestPass<T>
    where
        T: PassTest,
    {
        const NAME: &'static str = "unit test";

        type Output = ();
        type Error = Infallible;

        fn run(&mut self, ctx: &PassContext<T::Input>) -> Result<Self::Output, Self::Error> {
            self.test
                .check_pass_output(ctx.get_pass_output(self.target_pass));
            self.test.check_output_object(&*ctx.output.borrow());
            Ok(())
        }
    }
}
