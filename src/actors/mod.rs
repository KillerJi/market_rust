use actix::{fut::Ready, Actor, ActorFutureExt, AsyncContext, ContextFutureSpawner, WrapFuture};
use futures::Future;

pub(crate) mod block;
pub(crate) mod ws;

pub fn async_call<A, F, C, R>(a: &A, ctx: &mut A::Context, f: F, c: C)
where
    A: Actor,
    A::Context: AsyncContext<A>,
    R: 'static,
    F: Future<Output = R> + 'static,
    C: FnOnce(R, &mut A, &mut A::Context) -> Ready<()> + 'static,
{
    f.into_actor(a).then(c).wait(ctx);
}
