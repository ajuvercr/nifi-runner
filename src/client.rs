use feignhttp::feign;
use serde_json::Value;

use crate::models::{
    ConnectionEntity, ControllerServiceEntity, ControllerServiceTypesEntity, ProcessTypesEntity,
    ProcessorEntity,
};

const NIFI_URL: &str = "http://localhost:8091/nifi-api";

pub struct Nifi;

#[feign(url=NIFI_URL)]
impl Nifi {
    #[get("/flow/about")]
    pub async fn get_info() -> feignhttp::Result<Value> {}

    #[get("/flow/processor-types")]
    pub async fn list_types() -> feignhttp::Result<ProcessTypesEntity> {}

    #[get("/flow/controller-service-types")]
    pub async fn list_services() -> feignhttp::Result<ControllerServiceTypesEntity> {}

    #[post("/process-groups/{group}/controller-services")]
    pub async fn create_service(
        #[path] group: &str,
        #[body] body: Value,
    ) -> feignhttp::Result<ControllerServiceEntity> {
    }

    #[post("/process-groups/{group}/processors")]
    pub async fn create_processor(
        #[path] group: &str,
        #[body] body: Value,
    ) -> feignhttp::Result<ProcessorEntity> {
    }

    #[put("/processors/{id}")]
    pub async fn update_prcocessor(
        #[path] id: &str,
        #[body] body: &ProcessorEntity,
    ) -> feignhttp::Result<ProcessorEntity> {
    }

    #[delete("/processors/{id}")]
    pub async fn delete_processor(
        #[path] id: &str,
        #[query] version: u32,
    ) -> feignhttp::Result<Value> {
    }

    #[delete("/controller-services/{id}")]
    pub async fn delete_service(
        #[path] id: &str,
        #[query] version: u32,
    ) -> feignhttp::Result<Value> {
    }

    #[post("/process-groups/{id}/connections")]
    pub async fn create_conection(
        #[path] id: &str,
        #[body] body: ConnectionEntity,
    ) -> feignhttp::Result<Value> {
    }
}

impl Nifi {
    pub async fn new_processor(group: &str, ty: &str) -> feignhttp::Result<ProcessorEntity> {
        let body = serde_json::json!({
            "revision": {"version": 0},
            "component": {"type": ty}
        });

        Nifi::create_processor(group, body).await
    }

    pub async fn new_service(group: &str, ty: &str) -> feignhttp::Result<ControllerServiceEntity> {
        let body = serde_json::json!({
            "revision": {"version": 0},
            "component": {"type": ty}
        });

        Nifi::create_service(group, body).await
    }
}
