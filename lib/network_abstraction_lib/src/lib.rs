use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

pub use inventory;
use serde_json::Value;
pub use typed_request_macros;
pub use erased_serde;

use futures::Stream;
use serde::{Deserialize, Serialize};

pub mod erasure;
pub mod general;
pub mod typed;
pub mod typed_stream;

use typed::RouteInput;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync>>;

pub type BorrowedBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>;

pub trait AsyncFnWrapper<'a, A: 'a, B> {
    type Output;
    type Fut: Future<Output = Self::Output> + Send + Sync + 'a;
    fn call(&self, a: &'a A, b: B) -> Self::Fut;
}

impl<'a, A: 'a, B, F, Fut> AsyncFnWrapper<'a, A, B> for F
where
    F: Fn(&'a A, B) -> Fut,
    Fut: Future + Send + Sync + 'a,
{
    type Output = Fut::Output;
    type Fut = Fut;
    fn call(&self, a: &'a A, b: B) -> Self::Fut {
        self(a, b)
    }
}

pub enum MiddlewareAction<'a> {
    SkipPredicate,
    ReassignValue(&'a dyn IntoRequest),
    Continue,
    Next,
}

pub struct Router<S>
where
    S: Send + Sync,
{
    state: S,
    middleware:
        Option<Box<dyn for<'a> Fn(String, &'a dyn IntoRequest) -> MiddlewareAction + Send + Sync>>,
    registry: HashMap<String, Box<dyn HandlerType<S>>>,
}

impl<S: Send + Sync> Router<S> {
    pub fn new(state: S) -> Router<S> {
        Router {
            state,
            registry: HashMap::new(),
            middleware: None,
        }
    }
}

impl<S: Send + Sync + 'static> Router<S> {
    pub fn get_state(&self) -> &S {
        &self.state
    }

    pub fn get_state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    pub fn register_handler(
        &mut self,
        handler: impl HandlerType<S> + 'static,
    ) -> &mut Router<S> {
        self.registry.insert(
            handler
                .get_mapping()
                .unwrap_or(self.registry.len().to_string()),
            Box::new(handler),
        );
        self
    }

    pub fn register_typed<I>(
        &mut self,
        handler: impl typed::TypedHandler<S, Input = I, Output = I::Output> + 'static,
    ) -> &mut Router<S>
    where
        I: RouteInput<S> + Clone,
    {
        I::set(handler);
        let key = I::type_key();
        self.registry
            .insert(key, Box::new(typed::TypedRouteHandler::<S, I>::new()));
        self
    }

    pub fn register_typed_stream<I>(
        &mut self,
        handler: impl typed_stream::TypedStreamHandler<S, Input = I, Item = I::Item> + 'static,
    ) -> &mut Router<S>
    where
        I: typed_stream::StreamRouteInput<S> + Clone,
    {
        I::set(handler);
        let key = I::type_key();
        self.registry.insert(key, Box::new(typed_stream::TypedStreamRouteHandler::<S, I>::new()));
        self
    }
    pub async fn execute_typed<I>(&self, input: I) -> Result<I::Output, RouterErrors>
    where
        I: RouteInput<S>,
    {
        let handler = I::get().ok_or(RouterErrors::NoHandlerFound)?;
        Ok(handler.call(&self.state, input).await)
    }

    pub fn add_middleware<T>(&mut self, middleware: T)
    where
        T: for<'a> Fn(String, &'a dyn IntoRequest) -> MiddlewareAction + Send + Sync + 'static,
    {
        self.middleware = Some(Box::new(middleware));
    }

    pub fn map_router<F>(self, f: F) -> Router<S>
    where
        F: Fn(Router<S>) -> Router<S>,
    {
        f(self)
    }


    pub async fn execute_handler(
        &mut self,
        request: impl IntoRequest + 'static,
        handler_name: String,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>, RouterErrors> {
        if let Some(handler) = self.registry.get_mut(&handler_name) {
            let boxed_request: Box<dyn IntoRequest> = Box::new(request);
            Ok(handler.execute(&self.state, boxed_request).await)
        } else {
            Err(RouterErrors::NoHandlerFound)
        }
    }

    pub async fn execute_handler_typed<T: Any + Send + Sync + 'static>(
        &mut self,
        request: T,
        handler_name: String,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>, RouterErrors> {
        if let Some(handler) = self.registry.get_mut(&handler_name) {
            let boxed: Box<dyn Any + Send + Sync> = Box::new(request);
            let wrapped = handler.try_direct(boxed)?;
            Ok(handler.execute(&self.state, wrapped).await)
        } else {
            Err(RouterErrors::NoHandlerFound)
        }
    }

    pub async fn feed_bytes(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>, RouterErrors> {
        let state = &self.state;
        for (mapping, handler) in &mut self.registry {
            let mut request: &dyn IntoRequest = &BytesRequest::new(bytes.clone());

            if let Some(ref middleware) = self.middleware {
                match (middleware)(mapping.to_string(), request) {
                    MiddlewareAction::SkipPredicate => {
                        return Ok(handler.execute(state, request.clone_box()).await);
                    }
                    MiddlewareAction::ReassignValue(value) => request = value,
                    MiddlewareAction::Continue => {},
                    MiddlewareAction::Next => {continue},
                }
            }

            if let Ok(modified_request) = handler.try_predicate(request) {
                return Ok(handler.execute(state, modified_request).await);
            }
        }
        Err(RouterErrors::NoHandlerFound)
    }

    pub async fn feed_value(
        &mut self,
        value: serde_json::Value,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>, RouterErrors> {
        let state = &self.state;
        for (mapping, handler) in &mut self.registry {
            let mut request: &dyn IntoRequest = &ValueRequest::new(value.clone());

            if let Some(ref middleware) = self.middleware {
                match (middleware)(mapping.to_string(), request) {
                    MiddlewareAction::SkipPredicate => {
                        return Ok(handler.execute(state, request.clone_box()).await);
                    }
                    MiddlewareAction::ReassignValue(value) => request = value,
                    MiddlewareAction::Continue => {},
                    MiddlewareAction::Next => {continue},
                }
            }

            if let Ok(modified_request) = handler.try_predicate(request) {
                return Ok(handler.execute(state, modified_request).await);
            }
        }
        Err(RouterErrors::NoHandlerFound)
    }
}

#[derive(Debug)]

pub enum RouterErrors {
    NoHandlerFound,
    NotImplimented,
    Any(Box<dyn Error + Send + Sync>)
}

#[derive(Debug)]
pub enum ExtractorErrors {
    NotValidExtractor,
    FailedToExtract,
    Err(String),
}

pub trait IntoRequest: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn clone_box(&self) -> Box<dyn IntoRequest>;
}

pub trait IntoResponse<S>: Send + Sync {
    fn try_into_response(&self) -> Result<S, ExtractorErrors>;
    fn try_into_bytes(&self) -> Result<Vec<u8>, ExtractorErrors> {
        Err(ExtractorErrors::NotValidExtractor)
    }
    fn try_into_value(&self) -> Result<Value, ExtractorErrors> {
        Err(ExtractorErrors::NotValidExtractor)
    }
}

pub trait HandlerType<S>: Send + Sync
where
    S: Send + Sync,
{
    fn add_router(&self, router: &Router<S>);
    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors>;
    fn get_mapping(&self) -> Option<String>;
    fn mapping(self, mapping: String) -> Self
    where
        Self: Sized;
    fn execute<'a>(
        &mut self,
        state: &'a S,
        request: Box<dyn IntoRequest>,
    ) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>>;
    fn try_direct(
        &mut self,
        _request: Box<dyn Any + Send + Sync>,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        Err(RouterErrors::NotImplimented)
    }
}

#[derive(Clone)]
pub struct ValueRequest {
    pub value: serde_json::Value,
}

impl ValueRequest {
    fn new(value: serde_json::Value) -> ValueRequest {
        ValueRequest { value }
    }
}

impl IntoRequest for ValueRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct BytesRequest {
    pub bytes: Vec<u8>,
}

impl BytesRequest {
    fn new(bytes: Vec<u8>) -> BytesRequest {
        BytesRequest { bytes }
    }
}

impl IntoRequest for BytesRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(self.clone())
    }
}

pub trait ExtractResponse {
    fn extract<T: 'static>(&self) -> Result<T, ExtractorErrors>;
}

impl ExtractResponse for dyn IntoResponse<Box<dyn Any + Send + Sync>> {
    fn extract<T: 'static>(&self) -> Result<T, ExtractorErrors> {
        let boxed = self.try_into_response()?;
        boxed
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| ExtractorErrors::FailedToExtract)
    }
}

#[typed_request_macros::register_output]
#[derive(Default, Deserialize, Serialize, Clone, Debug)]
pub struct NoneResponse {}

impl IntoResponse<NoneResponse> for NoneResponse {
    fn try_into_response(&self) -> Result<NoneResponse, ExtractorErrors> {
        unimplemented!("Cannot convert NoneResponse into anything")
    }
}

#[typed_request_macros::register_output]
#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct ErrorResponse {
    pub error: String,
}

impl<S> IntoResponse<S> for ErrorResponse {
    fn try_into_response(&self) -> Result<S, ExtractorErrors> {
        Err(ExtractorErrors::Err(self.error.clone()))
    }
}

pub struct StringResponse(pub String);

impl From<String> for StringResponse {
    fn from(s: String) -> Self {
        StringResponse(s)
    }
}

impl IntoResponse<Box<dyn Any + Send + Sync>> for StringResponse {
    fn try_into_response(&self) -> Result<Box<dyn Any + Send + Sync>, ExtractorErrors> {
        Ok(Box::new(self.0.clone()))
    }
}

pub struct StreamResponse<Item> {
    inner: Mutex<Option<Pin<Box<dyn Stream<Item = Item> + Send + Sync>>>>,
}

impl<Item: Send + Sync + 'static> StreamResponse<Item> {
    pub fn new(stream: impl Stream<Item = Item> + Send + Sync + 'static) -> Self {
        StreamResponse {
            inner: Mutex::new(Some(Box::pin(stream))),
        }
    }
}

impl<R: Stream> From<R> for StreamResponse<<R as Stream>::Item>
where
    R: Send + Sync + 'static,
    <R as Stream>::Item: Send + Sync + 'static,
{
    fn from(stream: R) -> Self {
        StreamResponse::new(stream)
    }
}

impl<Item: Send + Sync + 'static> IntoResponse<Box<dyn Any + Send + Sync>>
    for StreamResponse<Item>
{
    fn try_into_response(&self) -> Result<Box<dyn Any + Send + Sync>, ExtractorErrors> {
        let taken = self
            .inner
            .lock()
            .unwrap()
            .take()
            .ok_or(ExtractorErrors::FailedToExtract)?;
        Ok(Box::new(taken) as Box<dyn Any + Send + Sync>)
    }
}

struct MapOutput<F, M> {
    f: F,
    map: M,
}

impl<F, M> MapOutput<F, M> {
    fn new(f: F, map: M) -> Self {
        MapOutput { f, map }
    }
}

impl<'a, A: 'a, B, F, M, R2> AsyncFnWrapper<'a, A, B> for MapOutput<F, M>
where
    F: AsyncFnWrapper<'a, A, B>,
    M: Fn(F::Output) -> R2 + Copy + Send + Sync + 'a,
{
    type Output = R2;
    type Fut = MapFuture<F::Fut, M>;
    fn call(&self, a: &'a A, b: B) -> Self::Fut {
        MapFuture {
            inner: self.f.call(a, b),
            map: self.map,
        }
    }
}

struct MapFuture<Fut, M> {
    inner: Fut,
    map: M,
}

impl<Fut, M, R2> Future for MapFuture<Fut, M>
where
    Fut: Future,
    M: Fn(Fut::Output) -> R2,
{
    type Output = R2;
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let inner = unsafe { Pin::new_unchecked(&mut this.inner) };
        match inner.poll(cx) {
            std::task::Poll::Ready(v) => std::task::Poll::Ready((this.map)(v)),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

pub trait FromWire: Sized {
    type Request: IntoRequest + Clone + 'static;
    type Error;
    fn from_wire(req: Self::Request) -> Result<Self, Self::Error>;
}

impl<T, S: Send + Sync + IntoResponse<T>, E: Send + Sync + IntoResponse<T>> IntoResponse<T>
    for Result<S, E>
{
    fn try_into_response(&self) -> Result<T, ExtractorErrors> {
        match self {
            Ok(result) => result.try_into_response(),
            Err(e) => e.try_into_response(),
        }
    }
}

#[macro_export]
macro_rules! owned_state {
    ($f:expr) => {
        move |state: &::std::sync::Arc<_>, req| {
            let state = ::std::sync::Arc::clone(state);
            async move { $f(&state, req).await }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    mod main {
        use std::sync::Arc;
        use crate::typed;

        use crate::{NoneResponse, general::string_type};

        use super::*;

        #[derive(Debug, serde::Deserialize, Clone)]
        struct MyPayload {
            #[serde(rename = "name")]
            _name: String,
            #[serde(rename = "count")]
            _count: u32,
        }

        impl FromWire for MyPayload {
            type Request = BytesRequest;
            type Error = serde_json::Error;
            fn from_wire(req: BytesRequest) -> Result<Self, Self::Error> {
                serde_json::from_slice(&req.bytes)
            }
        }

        impl FromWire for BytesRequest {
            type Request = BytesRequest;
            type Error = serde_json::Error;
            fn from_wire(req: BytesRequest) -> Result<Self, Self::Error> {
                Ok(BytesRequest { bytes: req.bytes })
            }
        }

        impl IntoResponse<Box<dyn Any + Send + Sync>> for NoneResponse {
            fn try_into_response(&self) -> Result<Box<dyn Any + Send + Sync>, ExtractorErrors> {
                Ok(Box::new(NoneResponse {}))
            }
        }

        fn test_example(_request: BytesRequest) -> NoneResponse {
            println!("ran this");
            NoneResponse {}
        }

        struct State {}

        impl State {
            fn new() -> State {
                State {}
            }
        }

        #[tokio::test]
        async fn simple() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));
            router.register_handler(string_type(
                |_state: &Arc<State>, req: Box<dyn IntoRequest>| async move {
                    let bytes_req = req
                        .as_any()
                        .downcast_ref::<BytesRequest>()
                        .expect("wrong request type for this handler");
                    Box::new(test_example(BytesRequest {
                        bytes: bytes_req.bytes.clone(),
                    })) as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>
                },
            ));
            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()).await {
                Ok(_) => assert!(true),
                Err(_) => println!("err"),
            }
        }

        #[tokio::test]
        async fn erasure() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));
            router.register_handler(
                erasure::erase::<_, BytesRequest, _, NoneResponse, Arc<State>>(
                    |_state: &Arc<State>, req: BytesRequest| async move { test_example(req) },
                ),
            );
            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()).await {
                Ok(_) => assert!(true),
                Err(_) => println!("err"),
            }
        }

        #[tokio::test]
        async fn erasure_match() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));
            router.register_handler(erasure::erase::<_, MyPayload, _, NoneResponse, Arc<State>>(
                |_state: &Arc<State>, payload: MyPayload| async move {
                    println!("got {payload:?}");
                    NoneResponse {}
                },
            ));
            let bytes = r#"{"name":"widget","count":3}"#.as_bytes();
            match router.feed_bytes(bytes.to_vec()).await {
                Ok(_) => assert!(true),
                Err(_) => panic!("expected handler to match valid payload"),
            }
        }

        #[tokio::test]
        async fn erasure_mismatch() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));
            router.register_handler(erasure::erase::<_, MyPayload, _, NoneResponse, Arc<State>>(
                |_state: &Arc<State>, payload: MyPayload| async move {
                    println!("got {payload:?}");
                    NoneResponse {}
                },
            ));
            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()).await {
                Ok(_) => panic!("expected no handler to match malformed payload"),
                Err(RouterErrors::NoHandlerFound) => {
                    println!("this was correctly rejected, no handler matched");
                }
                Err(_) => {
                    println!("encountered some other error");
                }
            }
        }

        use crate::register_output;
        use crate::typed::{RouteInput, typed_fn};

        #[derive(Debug, Clone, Serialize, serde::Deserialize)]
        struct Greeting(String);
        register_output!(Greeting, "Greeting");

        #[derive(serde::Deserialize, Clone)]
        struct NameInput {
            name: String,
        }

        
        #[typed_request_macros::typed_request]
        impl RouteInput<Arc<State>> for NameInput {
            type Output = Greeting;
        }


        #[tokio::test]
        async fn typed_dispatch_via_feed_value() {
            let mut router: Router<Arc<State>> = Router::new(Arc::new(State::new()));
            router.register_typed(typed_fn(|_s: &Arc<State>, i: NameInput| async move {
                Greeting(format!("hi {}", i.name))
            }));

            let value = serde_json::json!({ "name": "Person" });
            let resp = router.feed_value(value).await.ok().unwrap();
            let greeting: Greeting = resp.extract().unwrap();
            assert_eq!(greeting.0, "hi Person");
        }

        #[tokio::test]
        async fn typed_dispatch() {
            let mut router: Router<Arc<State>> = Router::new(Arc::new(State::new()));
            router.register_typed(typed_fn(|_s: &Arc<State>, i: NameInput| async move {
                Greeting(format!("hi {}", i.name))
            }));

            let out = router
                .execute_typed(NameInput { name: "Person".into() })
                .await
                .ok()
                .unwrap();
            assert_eq!(out.0, "hi Person");
        }
    }
}
