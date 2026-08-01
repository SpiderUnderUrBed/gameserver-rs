use std::os::linux::raw::stat;
use std::{any::Any, collections::HashMap};

use std::cell::RefCell;
use std::rc::Rc;

pub struct Router<S> {
    state: S,
    registry: HashMap<String, Box<dyn HandlerType<S>>>,
}
impl<S: Clone> Router<S> {
    pub fn new(state: S) -> Router<S> {
        Router {
            state,
            registry: HashMap::new(),
        }
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
    pub fn map_router<F>(self, f: F) -> Router<S>
    where
        F: Fn(Router<S>) -> Router<S>,
    {
        f(self)
    }
    pub fn execute_handler(
        &mut self,
        state: S,
        request: impl IntoRequest + 'static,
        handler_name: String,
    ) -> Result<Box<dyn IntoResponse>, RouterErrors> {
        if let Some(handler) = self.registry.get_mut(&handler_name) {
            let boxed_request: Box<dyn IntoRequest> = Box::new(request);
            Ok(handler.execute(state, boxed_request))
        } else {
            Err(RouterErrors::NoHandlerFound)
        }
    }
    pub fn feed_bytes(&mut self, bytes: Vec<u8>) -> Result<Box<dyn IntoResponse>, RouterErrors> {
        for (_, handler) in &mut self.registry {
            let request = BytesRequest::new(bytes.clone());
            if let Ok(modified_request) = handler.try_predicate(&request) {
                return Ok(handler.execute(self.state.clone(), modified_request));
            }
        }
        return Err(RouterErrors::NoHandlerFound);
    }
}
pub enum RouterErrors {
    NoHandlerFound,
}

#[derive(Clone)]
pub struct BytesRequest {
    bytes: Vec<u8>,
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
}

pub trait IntoRequest {
    fn as_any(&self) -> &dyn Any;
}
pub trait IntoResponse {}

pub trait HandlerType<S> {
    fn add_router(&self, router: &Router<S>);
    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors>;
    //fn try_predicate<T>(&mut self, request: T) -> Result<T, RouterErrors>;
    fn get_mapping(&self) -> Option<String>;
    fn map(self, mapping: String) -> Self
    where
        Self: Sized;
    fn execute(&mut self, state: S, request: Box<dyn IntoRequest>) -> Box<dyn IntoResponse>;
}

impl<S> HandlerType<S> for StringHandler<S> {
    fn add_router(&self, router: &Router<S>) {
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
    fn execute(&mut self, state: S, request: Box<dyn IntoRequest>) -> Box<dyn IntoResponse> {
        (self.function)(state, request)
    }
    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn map(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct StringHandler<AppState> {
    mapping: Option<String>,
    function: Box<dyn FnMut(AppState, Box<dyn IntoRequest>) -> Box<dyn IntoResponse>>,
}
pub fn string_type<F, AppState>(f: F) -> StringHandler<AppState>
where
    F: FnMut(AppState, Box<dyn IntoRequest>) -> Box<dyn IntoResponse> + 'static,
{
    StringHandler {
        function: Box::new(f),
        mapping: None,
    }
}

impl<S> HandlerType<S> for BytesHandler {
    fn add_router(&self, router: &Router<S>) {
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

    fn execute(&mut self, state: S, request: Box<dyn IntoRequest>) -> Box<dyn IntoResponse> {
        (self.function)(request)
    }
    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn map(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct BytesHandler {
    mapping: Option<String>,
    function: Box<dyn FnMut(Box<dyn IntoRequest>) -> Box<dyn IntoResponse>>,
}
pub fn bytes_type<F>(f: F) -> BytesHandler
where
    F: FnMut(Box<dyn IntoRequest>) -> Box<dyn IntoResponse> + 'static,
{
    BytesHandler {
        function: Box::new(f),
        mapping: None,
    }
}

impl<S> HandlerType<S> for NoneHandler {
    fn add_router(&self, router: &Router<S>) {
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

    fn execute(&mut self, state: S, request: Box<dyn IntoRequest>) -> Box<dyn IntoResponse> {
        (self.function)(request)
    }
    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }

    fn map(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
}

pub struct NoneHandler {
    mapping: Option<String>,
    function: Box<dyn FnMut(Box<dyn IntoRequest>) -> Box<dyn IntoResponse>>,
}
pub fn none_type<F>(f: F) -> NoneHandler
where
    F: FnMut(Box<dyn IntoRequest>) -> Box<dyn IntoResponse> + 'static,
{
    NoneHandler {
        function: Box::new(f),
        mapping: None,
    }
}

struct ErasedHandler<S> {
    inner: Box<dyn FnMut(&dyn IntoRequest) -> bool>,
    call: Box<dyn FnMut(S, Box<dyn IntoRequest>) -> Box<dyn IntoResponse>>,
    mapping: Option<String>,
}

trait FromWire: Sized {
    type Request: IntoRequest + Clone + 'static;
    type Error;
    fn from_wire(req: Self::Request) -> Result<Self, Self::Error>;
}

fn erase<F, S, R, AppState>(mut f: F) -> ErasedHandler<AppState>
where
    F: FnMut(AppState, S) -> R + 'static,
    S: FromWire + Clone + 'static,
    R: IntoResponse + 'static,
    AppState: 'static,
{
    let cached: Rc<RefCell<Option<S>>> = Rc::new(RefCell::new(None));
    let cached_for_predicate = cached.clone();

    ErasedHandler {
        inner: Box::new(move |req: &dyn IntoRequest| {
            let Some(t) = req.as_any().downcast_ref::<S::Request>() else {
                return false;
            };
            match S::from_wire(t.clone()) {
                Ok(s) => {
                    *cached_for_predicate.borrow_mut() = Some(s);
                    true
                }
                Err(_) => false,
            }
        }),
        call: Box::new(move |state: AppState, _req: Box<dyn IntoRequest>| {
            let s = cached
                .borrow_mut()
                .take()
                .expect("try_predicate already verified this");
            Box::new(f(state, s))
        }),
        mapping: None,
    }
}

impl<S> HandlerType<S> for ErasedHandler<S> {
    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        let concrete = request
            .as_any()
            .downcast_ref::<BytesRequest>()
            .ok_or(RouterErrors::NoHandlerFound)?;
        if (self.inner)(request) {
            Ok(Box::new(concrete.clone()))
        } else {
            Err(RouterErrors::NoHandlerFound)
        }
    }

    // }
    fn execute(&mut self, state: S, request: Box<dyn IntoRequest>) -> Box<dyn IntoResponse> {
        (self.call)(state, request)
    }
    fn get_mapping(&self) -> Option<String> {
        self.mapping.clone()
    }
    fn map(mut self, mapping: String) -> Self {
        self.mapping = Some(mapping);
        self
    }
    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }
}
#[derive(Default)]
struct CommonResponse {}
impl IntoResponse for CommonResponse {}

// macro_rules!  {
//     () => {

//     };
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
        // impl TryFrom<BytesRequest> for MyPayload {
        //     type Error = serde_json::Error;
        //     fn try_from(req: BytesRequest) -> Result<Self, Self::Error> {
        //         serde_json::from_slice(&req.bytes)
        //     }
        // }
        fn test_example(request: BytesRequest) -> CommonResponse {
            println!("ran this");
            CommonResponse {}
        }

        struct State {}
        impl State {
            fn new() -> State {
                State {}
            }
        }

        #[test]
        fn simple() {
            //assert!(true)
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));
            router.register_handler(string_type(
                |state: Arc<State>, req: Box<dyn IntoRequest>| -> Box<dyn IntoResponse> {
                    let bytes_req = req
                        .as_any()
                        .downcast_ref::<BytesRequest>()
                        .expect("wrong request type for this handler");
                    Box::new(test_example(BytesRequest {
                        bytes: bytes_req.bytes.clone(),
                    }))
                },
            ));
            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()) {
                Ok(_) => {
                    println!("ok");
                    assert!(true);
                }
                Err(_) => {
                    println!("err");
                }
            }
        }
        #[test]
        fn erasure() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));

            router.register_handler(erase::<_, BytesRequest, _, Arc<State>>(
                |state: Arc<State>, req: BytesRequest| -> CommonResponse { test_example(req) },
            ));

            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()) {
                Ok(_) => {
                    println!("ok");
                    assert!(true);
                }
                Err(_) => {
                    println!("err");
                }
            }
        }
        #[test]
        fn erasure_match() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));

            router.register_handler(erase::<_, MyPayload, _, Arc<State>>(
                |state: Arc<State>, payload: MyPayload| -> CommonResponse {
                    println!("got {payload:?}");
                    CommonResponse {}
                },
            ));

            let bytes = r#"{"name":"widget","count":3}"#.as_bytes();
            match router.feed_bytes(bytes.to_vec()) {
                Ok(_) => assert!(true),
                Err(_) => panic!("expected handler to match valid payload"),
            }
        }
        #[test]
        fn erasure_mismatch() {
            let router: &mut Router<Arc<State>> = &mut Router::new(Arc::new(State::new()));

            router.register_handler(erase::<_, MyPayload, _, Arc<State>>(
                |state: Arc<State>, payload: MyPayload| -> CommonResponse {
                    println!("got {payload:?}");
                    CommonResponse {}
                },
            ));

            let bytes = "test".as_bytes();
            match router.feed_bytes(bytes.to_vec()) {
                Ok(_) => panic!("expected no handler to match malformed payload"),
                Err(RouterErrors::NoHandlerFound) => {
                    println!("this was correctly rejected, no handler matched");
                }
            }
        }
    }
}
