use std::any::Any;

use futures::Stream;

use crate::general::ErrorResponse;
use crate::{AsyncFnWrapper, FromWire, HandlerType, IntoRequest, IntoResponse, MapOutput, Router, RouterErrors, StreamResponse, StringResponse};
use crate::BorrowedBoxFuture;

pub struct ErasedHandler<S> {
    inner: Box<dyn FnMut(&dyn IntoRequest) -> Option<Box<dyn IntoRequest>> + Send + Sync>,
    direct: Box<dyn Fn(Box<dyn Any + Send + Sync>) -> Option<Box<dyn IntoRequest>> + Send + Sync>,
    pub call: Box<
        dyn for<'a> Fn(&'a S, Box<dyn IntoRequest>) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>>
            + Send + Sync,
    >,
    mapping: Option<String>,
}

pub fn erase_stream_wrapper<F, S, R, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: for<'a> AsyncFnWrapper<'a, AppState, S, Output = R> + Send + Sync + 'static,
    S: FromWire + Clone + Send + Sync + 'static,
    R: Stream + Send + Sync + 'static,
    <R as Stream>::Item: Send + Sync + 'static,
    AppState: Send + Sync + 'static,
{
    erase::<F, S, R, StreamResponse<<R as Stream>::Item>, AppState>(f)
}

pub fn erase_stream_wrapper_result<F, S, Item, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: for<'a> AsyncFnWrapper<'a, AppState, S, Output = Result<StreamResponse<Item>, ErrorResponse>>
        + Send
        + Sync
        + 'static,
    S: FromWire + Clone + Send + Sync + 'static,
    Item: Send + Sync + 'static,
    AppState: Send + Sync + 'static,
{
    erase::<
        F,
        S,
        Result<StreamResponse<Item>, ErrorResponse>,
        Result<StreamResponse<Item>, ErrorResponse>,
        AppState,
    >(f)
}

pub fn erase<F, S, R, T, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: for<'a> AsyncFnWrapper<'a, AppState, S, Output = R> + Send + Sync + 'static,
    S: FromWire + Clone + Send + Sync + 'static,
    R: Into<T> + 'static,
    T: IntoResponse<Box<dyn Any + Send + Sync>> + 'static,
    AppState: Send + Sync + 'static,
{
    ErasedHandler {
        inner: Box::new(move |req: &dyn IntoRequest| {
            let t = req.as_any().downcast_ref::<S::Request>()?;
            S::from_wire(t.clone())
                .ok()
                .map(|s| Box::new(Erased(s)) as Box<dyn IntoRequest>)
        }),
        direct: Box::new(|any: Box<dyn Any + Send + Sync>| {
            any.downcast::<S>()
                .ok()
                .map(|s| Box::new(Erased(*s)) as Box<dyn IntoRequest>)
        }),
        call: Box::new(move |state, req: Box<dyn IntoRequest>| {
            let Erased(s) = *req
                .into_any()
                .downcast::<Erased<S>>()
                .expect("execute called with mismatched request type");
            let fut = f.call(state, s);
            Box::pin(async move {
                let response: T = fut.await.into();
                Box::new(response) as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>
            })
        }),
        mapping: None,
    }
}

struct Erased<T>(T);

impl<T: 'static + Send + Sync + Clone> IntoRequest for Erased<T> {
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
    S: Send + Sync,
{
    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        (self.inner)(request).ok_or(RouterErrors::NoHandlerFound)
    }

    fn execute<'a>(
        &mut self,
        state: &'a S,
        request: Box<dyn IntoRequest>,
    ) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> {
        (self.call)(state, request)
    }
    
    fn try_direct(                                                    
        &mut self,
        request: Box<dyn Any + Send + Sync>,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        (self.direct)(request).ok_or(RouterErrors::NoHandlerFound)
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


pub fn erase_string_wrapper<F, S, R, AppState>(f: F) -> ErasedHandler<AppState>
where
    F: for<'a> AsyncFnWrapper<'a, AppState, S, Output = R> + Send + Sync + 'static,
    S: FromWire + Clone + Send + Sync + 'static,
    R: serde::Serialize + Send + Sync + 'static,
    AppState: Send + Sync + 'static,
{
    erase::<_, S, StringResponse, StringResponse, AppState>(MapOutput::new(
        f,
        |value: R| {
            let json = serde_json::to_string(&value)
                .unwrap_or_else(|e| format!(r#"{{"error":"serialization failed: {}"}}"#, e));
            StringResponse(json)
        },
    ))
}
