use std::{any::Any, pin::Pin, sync::{Arc, OnceLock}};

use serde::de::DeserializeOwned;

use crate::{AsyncFnWrapper, BorrowedBoxFuture, BytesRequest, ErrorResponse, HandlerType, IntoRequest, IntoResponse, Router, RouterErrors, StreamResponse, ValueRequest, typed::TypedRequest};

pub trait TypedStreamHandler<S>: Send + Sync {
    type Input: DeserializeOwned + Send + Sync + 'static;
    type Item: Send + Sync + 'static;

    fn call<'a>(
        &'a self,
        state: &'a S,
        input: Self::Input,
    ) -> BorrowedBoxFuture<'a, StreamResponse<Self::Item>>;
}

pub struct TypedStreamFn<F, In, Item> {
    f: F,
    _marker: std::marker::PhantomData<fn(In) -> Item>,
}

pub fn typed_stream_fn<S, F, In, Item>(f: F) -> TypedStreamFn<F, In, Item>
where
    S: Send + Sync,
    In: DeserializeOwned + Send + Sync + 'static,
    Item: Send + Sync + 'static,
    F: for<'a> AsyncFnWrapper<'a, S, In, Output = StreamResponse<Item>> + Send + Sync,
{
    TypedStreamFn { f, _marker: std::marker::PhantomData }
}

impl<S, F, In, Item> TypedStreamHandler<S> for TypedStreamFn<F, In, Item>
where
    S: Send + Sync,
    In: DeserializeOwned + Send + Sync + 'static,
    Item: Send + Sync + 'static,
    F: for<'a> AsyncFnWrapper<'a, S, In, Output = StreamResponse<Item>> + Send + Sync,
{
    type Input = In;
    type Item = Item;

    fn call<'a>(&'a self, state: &'a S, input: Self::Input) -> BorrowedBoxFuture<'a, StreamResponse<Item>> {
        Box::pin(self.f.call(state, input))
    }
}

pub trait StreamRouteInput<S>: DeserializeOwned + Send + Sync + 'static
where
    S: Send + Sync + 'static,
{
    type Item: Send + Sync + 'static;

    fn slot() -> &'static OnceLock<Arc<dyn TypedStreamHandler<S, Input = Self, Item = Self::Item>>>;

    fn type_key() -> String {
        std::any::type_name::<Self>().to_string()
    }

    fn set(handler: impl TypedStreamHandler<S, Input = Self, Item = Self::Item> + 'static) {
        let _ = Self::slot().set(Arc::new(handler));
    }

    fn get() -> Option<Arc<dyn TypedStreamHandler<S, Input = Self, Item = Self::Item>>> {
        Self::slot().get().cloned()
    }
}
pub(crate) struct TypedStreamRouteHandler<S, I> {
    _marker: std::marker::PhantomData<fn(S, I)>,
}

impl<S, I> TypedStreamRouteHandler<S, I> {
    pub(crate) fn new() -> Self {
        TypedStreamRouteHandler { _marker: std::marker::PhantomData }
    }
}

impl<S, I> HandlerType<S> for TypedStreamRouteHandler<S, I>
where
    S: Send + Sync + 'static,
    I: StreamRouteInput<S> + Clone,
{
    fn add_router(&self, _router: &Router<S>) {}

    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        if let Some(bytes_req) = request.as_any().downcast_ref::<BytesRequest>() {
            if let Ok(parsed) = serde_json::from_slice::<I>(&bytes_req.bytes) {
                return Ok(Box::new(TypedRequest(parsed)));
            }
        } else if let Some(value_req) = request.as_any().downcast_ref::<ValueRequest>() {
            if let Ok(parsed) = serde_json::from_value::<I>(value_req.value.clone()) {
                return Ok(Box::new(TypedRequest(parsed)));
            }
        }
        Err(RouterErrors::NoHandlerFound)
    }

    fn get_mapping(&self) -> Option<String> { None }

    fn mapping(self, _mapping: String) -> Self where Self: Sized { self }

    fn execute<'a>(
        &mut self,
        state: &'a S,
        request: Box<dyn IntoRequest>,
    ) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> {
        Box::pin(async move {
            let input = match request.into_any().downcast::<TypedRequest<I>>() {
                Ok(typed) => typed.0,
                Err(_) => {
                    return Box::new(ErrorResponse {
                        error: "typed stream handler received the wrong request wrapper".into(),
                    }) as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>;
                }
            };

            match I::get() {
                Some(handler) => {
                    let stream_response = handler.call(state, input).await;
                    Box::new(stream_response) as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>
                }
                None => Box::new(ErrorResponse {
                    error: "no typed stream handler registered for this input type".into(),
                }) as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>,
            }
        })
    }
}

pub fn typed_stream_fn_result<S, F, In, Item, E>(f: F) -> TypedStreamFn<MapStreamResult<F, E>, In, Item>
where
    S: Send + Sync,
    In: DeserializeOwned + Send + Sync + 'static,
    Item: Send + Sync + 'static,
    E: Send + Sync + 'static,
    F: for<'a> AsyncFnWrapper<'a, S, In, Output = Result<StreamResponse<Item>, E>> + Send + Sync,
{
    TypedStreamFn {
        f: MapStreamResult { f, _marker: std::marker::PhantomData },
        _marker: std::marker::PhantomData,
    }
}

pub struct MapStreamResult<F, E> {
    f: F,
    _marker: std::marker::PhantomData<fn() -> E>,
}

impl<'a, A: 'a, B, F, Item, E> AsyncFnWrapper<'a, A, B> for MapStreamResult<F, E>
where
    F: AsyncFnWrapper<'a, A, B, Output = Result<StreamResponse<Item>, E>>,
    Item: Send + Sync + 'static,
{
    type Output = StreamResponse<Item>;
    type Fut = MapStreamResultFuture<F::Fut>;
    fn call(&self, a: &'a A, b: B) -> Self::Fut {
        MapStreamResultFuture { inner: self.f.call(a, b) }
    }
}

pub struct MapStreamResultFuture<Fut> {
    inner: Fut,
}

impl<Fut, Item, E> Future for MapStreamResultFuture<Fut>
where
    Fut: Future<Output = Result<StreamResponse<Item>, E>>,
    Item: Send + Sync + 'static,
{
    type Output = StreamResponse<Item>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let inner = unsafe { Pin::new_unchecked(&mut this.inner) };
        match inner.poll(cx) {
            std::task::Poll::Ready(Ok(sr)) => std::task::Poll::Ready(sr),
            std::task::Poll::Ready(Err(_e)) => {
                std::task::Poll::Ready(StreamResponse::new(futures::stream::empty()))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}