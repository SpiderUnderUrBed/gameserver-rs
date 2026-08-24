use std::any::Any;

use serde::Serialize;

use crate::BoxFuture;
use crate::{
    BorrowedBoxFuture, BytesRequest, ExtractorErrors, HandlerType, IntoRequest, IntoResponse,
    Router, RouterErrors,
};

impl<S> HandlerType<S> for BytesHandler
where
    S: Send + Sync + Clone,
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

    fn execute<'a>(
        &mut self,
        _state: &'a S,
        request: Box<dyn IntoRequest>,
    ) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> {
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
    function: Box<
        dyn FnMut(
                Box<dyn IntoRequest>,
            ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>>
            + Send
            + Sync,
    >,
}

pub fn bytes_type<F, Fut>(mut f: F) -> BytesHandler
where
    F: FnMut(Box<dyn IntoRequest>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> + Send + Sync + 'static,
{
    BytesHandler {
        function: Box::new(move |req| Box::pin(f(req))),
        mapping: None,
    }
}

impl<S> HandlerType<S> for NoneHandler
where
    S: Send + Sync + Clone,
{
    fn add_router(&self, _router: &Router<S>) {
        todo!()
    }

    fn try_predicate(
        &mut self,
        _request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        Err(RouterErrors::NoHandlerFound)
    }

    fn execute<'a>(
        &mut self,
        _state: &'a S,
        request: Box<dyn IntoRequest>,
    ) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> {
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
    function: Box<
        dyn FnMut(
                Box<dyn IntoRequest>,
            ) -> BoxFuture<Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>>
            + Send
            + Sync,
    >,
}

pub fn none_type<F, Fut>(mut f: F) -> NoneHandler
where
    F: FnMut(Box<dyn IntoRequest>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> + Send + Sync + 'static,
{
    NoneHandler {
        function: Box::new(move |req| Box::pin(f(req))),
        mapping: None,
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
    pub error: String,
}

impl<S> IntoResponse<S> for ErrorResponse {
    fn try_into_response(&self) -> Result<S, ExtractorErrors> {
        Err(ExtractorErrors::Err(self.error.clone()))
    }
}
