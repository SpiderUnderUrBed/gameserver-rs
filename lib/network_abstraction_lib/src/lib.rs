use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use futures::Stream;
use serde::Serialize;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;


pub enum MiddlewareAction<'a> {
    SkipPredicate,
    ReassignValue(&'a dyn IntoRequest),
    Continue
}
pub struct Router<S>
where
    S: Send + Clone,
{
    state: S,
    middleware: Option<
        Box<dyn for<'a> Fn(String, &'a dyn IntoRequest) -> MiddlewareAction + Send>,
    >,
    registry: HashMap<String, Box<dyn HandlerType<S>>>,
}

impl<S: Clone + Send> Router<S> {
    pub fn new(state: S) -> Router<S> {
        Router {
            state,
            registry: HashMap::new(),
            middleware: None,
        }
    }

    pub fn get_state(&self) -> S {
        self.state.clone()
    }

    pub fn get_state_mut(&mut self) -> S {
        self.state.clone()
    }


    pub fn register_handler(&mut self, handler: impl HandlerType<S> + 'static) -> &mut Router<S> {
        self.registry.insert(
            handler
                .get_mapping()
                .unwrap_or(self.registry.len().to_string()),
            Box::new(handler),
        );
        self
    }

    pub fn add_middleware<T>(&mut self, middleware: T)
    where
        T: for<'a> Fn(String, &'a dyn IntoRequest) -> MiddlewareAction
            + Send
            + 'static,
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
        state: S,
        request: impl IntoRequest + 'static,
        handler_name: String,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send>>>, RouterErrors> {
        if let Some(handler) = self.registry.get_mut(&handler_name) {
            let boxed_request: Box<dyn IntoRequest> = Box::new(request);
            Ok(handler.execute(state, boxed_request).await)
        } else {
            Err(RouterErrors::NoHandlerFound)
        }
    }

    pub async fn feed_bytes(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send>>>, RouterErrors> {
        for (mapping, handler) in &mut self.registry {
            let mut request: &dyn IntoRequest = &BytesRequest::new(bytes.clone());

            if let Some(ref middleware) = self.middleware {
                match (middleware)(mapping.to_string(), request) {
                    MiddlewareAction::SkipPredicate => return Ok(handler.execute(self.state.clone(), request.clone_box()).await),
                    MiddlewareAction::ReassignValue(value) => request = value,
                    MiddlewareAction::Continue => continue,
                }
            }

            println!("trying {}", mapping);
            if let Ok(modified_request) = handler.try_predicate(request) {
                println!("passed predicate");
                return Ok(handler.execute(self.state.clone(), modified_request).await);
            } else {
                println!("failed predicate");
            }
        }
        Err(RouterErrors::NoHandlerFound)
    }

    pub async fn feed_value(
        &mut self,
        value: serde_json::Value,
    ) -> Result<Box<dyn IntoResponse<Box<dyn Any + Send>>>, RouterErrors> {
        for (mapping, handler) in &mut self.registry {
            let mut request: &dyn IntoRequest = &ValueRequest::new(value.clone());

            if let Some(ref middleware) = self.middleware {
                match (middleware)(mapping.to_string(), request) {
                    MiddlewareAction::SkipPredicate => return Ok(handler.execute(self.state.clone(), request.clone_box()).await),
                    MiddlewareAction::ReassignValue(value) => request = value,
                    MiddlewareAction::Continue => continue,
                }
            }

            if let Ok(modified_request) = handler.try_predicate(request) {
                return Ok(handler.execute(self.state.clone(), modified_request).await);
            }
        }
        Err(RouterErrors::NoHandlerFound)
    }
}

pub enum RouterErrors {
    NoHandlerFound,
}
pub enum ExtractorErrors {
    NotValidExtractor,
    FailedToExtract,
    Err(String)
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

pub trait IntoRequest: Send {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn clone_box(&self) -> Box<dyn IntoRequest>;
}

pub trait IntoResponse<S>: Send {
    fn try_into_response(&self) -> Result<S, ExtractorErrors>;
}

pub trait ExtractResponse {
    fn extract<T: 'static>(&self) -> Result<T, ExtractorErrors>;
}

impl ExtractResponse for dyn IntoResponse<Box<dyn Any + Send>> {
    fn extract<T: 'static>(&self) -> Result<T, ExtractorErrors> {
        let boxed = self.try_into_response()?;
        boxed
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| ExtractorErrors::FailedToExtract)
    }
}


pub trait HandlerType<S>: Send
where
    S: Send + Clone,
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
    fn execute(
        &mut self,
        state: S,
        request: Box<dyn IntoRequest>,
    ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>>;
}
// Ok(request.clone_box())
impl<S: Send> HandlerType<S> for AnyHandler<S>
where
    S: Send + Clone,
{
    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }

    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        Ok(request.clone_box())
    }

    fn execute(
        &mut self,
        state: S,
        request: Box<dyn IntoRequest>,
    ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> {
        (self.function)(state, request)
    }

    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn mapping(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct AnyHandler<AppState> {
    mapping: Option<String>,
    function:
        Box<dyn FnMut(AppState, Box<dyn IntoRequest>) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send>,
}

pub fn any_type<F, Fut, AppState>(mut f: F) -> AnyHandler<AppState>
where
    F: FnMut(AppState, Box<dyn IntoRequest>) -> Fut + Send + 'static,
    Fut: Future<Output = Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send + 'static,
{
    AnyHandler {
        function: Box::new(move |state, req| Box::pin(f(state, req))),
        mapping: None,
    }
}

impl<S: Send> HandlerType<S> for StringHandler<S>
where
    S: Send + Clone,
{
    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }

    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        let concrete = request
            .as_any()
            .downcast_ref::<BytesRequest>()
            .ok_or(RouterErrors::NoHandlerFound)?;
        Ok(Box::new(concrete.clone()))
    }

    fn execute(
        &mut self,
        state: S,
        request: Box<dyn IntoRequest>,
    ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> {
        (self.function)(state, request)
    }

    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn mapping(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct StringHandler<AppState> {
    mapping: Option<String>,
    function:
        Box<dyn FnMut(AppState, Box<dyn IntoRequest>) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send>,
}

pub fn string_type<F, Fut, AppState>(mut f: F) -> StringHandler<AppState>
where
    F: FnMut(AppState, Box<dyn IntoRequest>) -> Fut + Send + 'static,
    Fut: Future<Output = Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send + 'static,
{
    StringHandler {
        function: Box::new(move |state, req| Box::pin(f(state, req))),
        mapping: None,
    }
}

pub struct StringResponse(pub String);

impl From<String> for StringResponse {
    fn from(s: String) -> Self {
        StringResponse(s)
    }
}

impl IntoResponse<Box<dyn Any + Send>> for StringResponse {
    fn try_into_response(&self) -> Result<Box<dyn Any + Send>, ExtractorErrors> {
        Ok(Box::new(self.0.clone()))
    }
}

pub fn erase_string_wrapper<F, Fut, S, R, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: Fn(AppState, S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    S: FromWire + Clone + Send + 'static,
    R: serde::Serialize + Send + 'static,
    AppState: Send + 'static,
{
    erase::<_, _, S, StringResponse, StringResponse, AppState>(move |state: AppState, s: S| {
        let fut = f(state, s);
        async move {
            let value: R = fut.await;
            let json = serde_json::to_string(&value)
                .unwrap_or_else(|e| format!(r#"{{"error":"serialization failed: {}"}}"#, e));
            StringResponse(json)
        }
    })
}


pub struct StreamResponse<Item> {
    inner: Mutex<Option<Pin<Box<dyn Stream<Item = Item> + Send>>>>,
}

impl<Item: Send + 'static> StreamResponse<Item> {
    pub fn new(stream: impl Stream<Item = Item> + Send + 'static) -> Self {
        StreamResponse {
            inner: Mutex::new(Some(Box::pin(stream))),
        }
    }
}

impl<R> From<R> for StreamResponse<R::Item>
where
    R: Stream + Send + 'static,
    R::Item: Send + 'static,
{
    fn from(stream: R) -> Self {
        StreamResponse::new(stream)
    }
}

impl<Item: Send + 'static> IntoResponse<Box<dyn Any + Send>> for StreamResponse<Item> {
    fn try_into_response(&self) -> Result<Box<dyn Any + Send>, ExtractorErrors> {
        let taken = self
            .inner
            .lock()
            .unwrap()
            .take()
            .ok_or(ExtractorErrors::FailedToExtract)?; 
        Ok(Box::new(taken) as Box<dyn Any + Send>)
    }
}

pub fn erase_stream_wrapper<F, Fut, S, R, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: Fn(AppState, S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    S: FromWire + Clone + Send + 'static,
    R: Stream + Send + 'static,
    R::Item: Send + 'static,
    AppState: 'static,
{
    erase::<F, Fut, S, R, StreamResponse<R::Item>, AppState>(f)
}

pub fn erase_stream_wrapper_result<F, Fut, S, Item, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: Fn(AppState, S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<StreamResponse<Item>, ErrorResponse>> + Send + 'static,
    S: FromWire + Clone + Send + 'static,
    Item: Send + 'static,
    AppState: 'static,
{
    erase::<
        F,
        Fut,
        S,
        Result<StreamResponse<Item>, ErrorResponse>,
        Result<StreamResponse<Item>, ErrorResponse>,
        AppState,
    >(f)
}

// impl <S>IntoResponse<S> for StreamResponse<S>{
//     fn try_into_response(&self) -> Result<S, ExtractorErrors> {
//         Err(ExtractorErrors::NotValidExtractor)
//     }
// }

impl<S> HandlerType<S> for BytesHandler
where
    S: Send + Clone,
{
    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }

    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        let concrete = request
            .as_any()
            .downcast_ref::<BytesRequest>()
            .ok_or(RouterErrors::NoHandlerFound)?;
        Ok(Box::new(concrete.clone()))
    }

    fn execute(
        &mut self,
        _state: S,
        request: Box<dyn IntoRequest>,
    ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> {
        (self.function)(request)
    }

    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn mapping(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct BytesHandler {
    mapping: Option<String>,
    function: Box<dyn FnMut(Box<dyn IntoRequest>) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send>,
}

pub fn bytes_type<F, Fut>(mut f: F) -> BytesHandler
where
    F: FnMut(Box<dyn IntoRequest>) -> Fut + Send + 'static,
    Fut: Future<Output = Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send + 'static,
{
    BytesHandler {
        function: Box::new(move |req| Box::pin(f(req))),
        mapping: None,
    }
}

impl<S> HandlerType<S> for NoneHandler
where
    S: Send + Clone,
{
    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }

    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        Err(RouterErrors::NoHandlerFound)
    }

    fn execute(
        &mut self,
        _state: S,
        request: Box<dyn IntoRequest>,
    ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> {
        (self.function)(request)
    }

    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn mapping(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct NoneHandler {
    mapping: Option<String>,
    function: Box<dyn FnMut(Box<dyn IntoRequest>) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send>,
}

pub fn none_type<F, Fut>(mut f: F) -> NoneHandler
where
    F: FnMut(Box<dyn IntoRequest>) -> Fut + Send + 'static,
    Fut: Future<Output = Box<dyn IntoResponse<Box<dyn Any + Send>>>> + Send + 'static,
{
    NoneHandler {
        function: Box::new(move |req| Box::pin(f(req))),
        mapping: None,
    }
}

pub struct ErasedHandler<S> {
    inner: Box<dyn FnMut(&dyn IntoRequest) -> Option<Box<dyn IntoRequest>> + Send>,
    pub call: Box<
        dyn Fn(S, Box<dyn IntoRequest>) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>>
            + Send
            + Sync,
    >,
    mapping: Option<String>,
}

pub fn erase<F, Fut, S, R, T, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: Fn(AppState, S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    S: FromWire + Clone + Send + 'static,
    R: Into<T> + 'static,
    T: IntoResponse<Box<dyn Any + Send>> + 'static,
    AppState: 'static,
{
    ErasedHandler {
        inner: Box::new(move |req: &dyn IntoRequest| {
            let t = req.as_any().downcast_ref::<S::Request>()?;
            S::from_wire(t.clone())
                .ok()
                .map(|s| Box::new(Erased(s)) as Box<dyn IntoRequest>)
        }),
        call: Box::new(move |state: AppState, req: Box<dyn IntoRequest>| {
            let Erased(s) = *req
                .into_any()
                .downcast::<Erased<S>>()
                .expect("execute called with mismatched request type");
            let fut = f(state, s);
            Box::pin(async move {
                let response: T = fut.await.into();
                Box::new(response) as Box<dyn IntoResponse<Box<dyn Any + Send>>>
            })
        }),
        mapping: None,
    }
}

pub trait FromWire: Sized {
    type Request: IntoRequest + Clone + 'static;
    type Error;
    fn from_wire(req: Self::Request) -> Result<Self, Self::Error>;
}



struct Erased<T>(T);

impl<T: 'static + Send + Clone> IntoRequest for Erased<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(Erased(self.0.clone()))
    }
}

impl<S> HandlerType<S> for ErasedHandler<S>
where
    S: Send + Clone,
{
    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        (self.inner)(request).ok_or(RouterErrors::NoHandlerFound)
    }

    fn execute(
        &mut self,
        state: S,
        request: Box<dyn IntoRequest>,
    ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send>>>> {
        (self.call)(state, request)
    }

    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn mapping(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }

    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }
}

impl<T, S: Send + IntoResponse<T>, E: Send + IntoResponse<T>> IntoResponse<T> for Result<S, E> {
    fn try_into_response(&self) -> Result<T, ExtractorErrors> {
        match self {
            Ok(result) => result.try_into_response(),
            Err(e) => e.try_into_response()
        }
    }
}

#[derive(Default, Serialize)]
pub struct NoneResponse {}

impl IntoResponse<NoneResponse> for NoneResponse {
    fn try_into_response(&self) -> Result<NoneResponse, ExtractorErrors> {
        unimplemented!("Cannot convert NoneResponse into anything")
    }
}

#[derive(Default, Serialize)]
pub struct ErrorResponse {
    pub error: String
}


impl <S>IntoResponse<S> for ErrorResponse {
    fn try_into_response(&self) -> Result<S, ExtractorErrors> {
        Err(ExtractorErrors::Err(self.error.clone()))
    }
}

// impl <S: Serialize>IntoResponse<S> for ErrorResponse {
//     fn try_into_response(&self) -> Result<String, ExtractorErrors> {
//         unimplemented!("not valid");
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    mod main {
        use std::sync::Arc;

        use super::*;

        #[derive(Debug, serde::Deserialize, Clone)]
        struct MyPayload {
            name: String,
            count: u32,
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

        impl IntoResponse<Box<dyn Any + Send>> for NoneResponse {
            fn try_into_response(&self) -> Result<Box<dyn Any + Send>, ExtractorErrors> {
                Ok(Box::new(NoneResponse {}))
            }
        }

        fn test_example(request: BytesRequest) -> NoneResponse {
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
                |_state: Arc<State>, req: Box<dyn IntoRequest>| async move {
                    let bytes_req = req
                        .as_any()
                        .downcast_ref::<BytesRequest>()
                        .expect("wrong request type for this handler");
                    Box::new(test_example(BytesRequest {
                        bytes: bytes_req.bytes.clone(),
                    })) as Box<dyn IntoResponse<Box<dyn Any + Send>>>
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
            router.register_handler(erase::<_, _, BytesRequest, _, NoneResponse, Arc<State>>(
                |_state: Arc<State>, req: BytesRequest| async move { test_example(req) },
            ));
            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()).await {
                Ok(_) => assert!(true),
                Err(_) => println!("err"),
            }
        }

        #[tokio::test]
        async fn erasure_match() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));
            router.register_handler(erase::<_, _, MyPayload, _, NoneResponse, Arc<State>>(
                |_state: Arc<State>, payload: MyPayload| async move {
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
            router.register_handler(erase::<_, _, MyPayload, _, NoneResponse, Arc<State>>(
                |_state: Arc<State>, payload: MyPayload| async move {
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
            }
        }
    }
}
