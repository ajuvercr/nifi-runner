use feignhttp::{feign, Feign};
use serde_json::Value;

use crate::models::{
    ConnectionEntity, ControllerServiceEntity, ControllerServiceTypesEntity, FlowEntity,
    PortEntity, PortsEntity, ProcessGroupEntity, ProcessTypesEntity, ProcessorEntity, Variable,
    VariableDTO, VariableRegistryEntity,
};

const NIFI_URL: &str = "http://localhost:8091/nifi-api";

#[derive(Feign, clap::Args, Debug)]
pub struct Nifi {
    #[arg(short, long, default_value_t = String::from(NIFI_URL))]
    nifi: String,
    #[arg(short, long, default_value_t = String::from("root"))]
    #[feign_path]
    pub group: String,
}

impl Nifi {
    pub fn change_group(&self, group: &str) -> Self {
        Nifi {
            nifi: self.nifi.clone(),
            group: group.to_string(),
        }
    }
}

pub enum PortType {
    Input,
    Output,
}
impl std::fmt::Display for PortType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortType::Input => write!(f, "input"),
            PortType::Output => write!(f, "output"),
        }
    }
}

#[feign(url=self.nifi)]
impl Nifi {
    #[get("/flow/about")]
    pub async fn get_info(&self) -> feignhttp::Result<Value> {}

    #[get("/flow/processor-types")]
    pub async fn list_types(&self) -> feignhttp::Result<ProcessTypesEntity> {}

    #[get("/flow/controller-service-types")]
    pub async fn list_services(&self) -> feignhttp::Result<ControllerServiceTypesEntity> {}

    #[get("/process-groups/{group}")]
    pub async fn get_process_group(&self) -> feignhttp::Result<ProcessGroupEntity> {}

    #[get("/flow/process-groups/{group}/controller-services")]
    pub async fn list_active_services(&self) -> feignhttp::Result<Value> {}

    #[get("/process-groups/{group}/processors")]
    pub async fn list_active_processors(&self) -> feignhttp::Result<Value> {}

    #[post("/process-groups/{group}/controller-services")]
    pub async fn create_service(
        &self,
        #[body] body: Value,
    ) -> feignhttp::Result<ControllerServiceEntity> {
    }

    #[post("/process-groups/{group}/processors")]
    pub async fn create_processor(
        &self,
        #[body] body: Value,
    ) -> feignhttp::Result<ProcessorEntity> {
    }

    #[post("/process-groups/{group}/process-groups")]
    pub async fn create_process_group(
        &self,
        #[body] body: Value,
    ) -> feignhttp::Result<ProcessGroupEntity> {
    }

    #[get("/process-groups/{group}/variable-registry")]
    pub async fn get_variables(&self) -> feignhttp::Result<VariableRegistryEntity> {}

    #[post("/process-groups/{group}/variable-registry/update-requests")]
    pub async fn update_variable_request(
        &self,
        #[body] body: VariableRegistryEntity,
    ) -> feignhttp::Result<Value> {
    }

    #[post("/process-groups/{group}/templates/upload")]
    pub async fn api_upload_template(
        &self,
        #[multi] template: String,
    ) -> feignhttp::Result<String> {
    }

    #[post("/process-groups/{group}/template-instance")]
    pub async fn api_instantiate_template(
        &self,
        #[body] body: Value,
    ) -> feignhttp::Result<FlowEntity> {
    }

    #[get("/process-groups/{group}/{ty}-ports")]
    pub async fn get_ports(&self, #[path] ty: PortType) -> feignhttp::Result<PortsEntity> {}

    #[post("/process-groups/{group}/{ty}-ports")]
    pub async fn create_port(
        &self,
        #[path] ty: PortType,
        #[body] body: Value,
    ) -> feignhttp::Result<PortEntity> {
    }

    #[put("/processors/{id}")]
    pub async fn update_prcocessor(
        &self,
        #[path] id: &str,
        #[body] body: &ProcessorEntity,
    ) -> feignhttp::Result<ProcessorEntity> {
    }

    #[delete("/processors/{id}")]
    pub async fn delete_processor(
        &self,
        #[path] id: &str,
        #[query] version: u32,
    ) -> feignhttp::Result<Value> {
    }

    #[delete("/controller-services/{id}")]
    pub async fn delete_service(
        &self,
        #[path] id: &str,
        #[query] version: u32,
    ) -> feignhttp::Result<Value> {
    }

    #[delete("/templates/{id}")]
    pub async fn delete_template(&self, #[path] id: &str) -> feignhttp::Result<Value> {}

    #[post("/process-groups/{group}/connections")]
    pub async fn create_conection(
        &self,
        #[body] body: ConnectionEntity,
    ) -> feignhttp::Result<Value> {
    }
}

impl Nifi {
    pub async fn new_process_group(&self, name: &str) -> feignhttp::Result<ProcessGroupEntity> {
        let body = serde_json::json!({
            "revision": {"version": 0},
            "component": {"name": name}
        });
        self.create_process_group(body).await
    }

    pub async fn new_processor(&self, ty: &str) -> feignhttp::Result<ProcessorEntity> {
        let body = serde_json::json!({
            "revision": {"version": 0},
            "component": {"type": ty}
        });

        self.create_processor(body).await
    }

    pub async fn new_service(&self, ty: &str) -> feignhttp::Result<ControllerServiceEntity> {
        let body = serde_json::json!({
            "revision": {"version": 0},
            "component": {"type": ty}
        });

        self.create_service(body).await
    }

    pub async fn new_port(&self, ty: PortType, name: &str) -> feignhttp::Result<PortEntity> {
        let ty_str = ty.to_string();
        let body = serde_json::json!({
            "revision": {"version": 0},
            "portType": ty_str,
            "component": {"name": name}
        });
        self.create_port(ty, body).await
    }

    pub async fn set_variables(
        &self,
        vars: impl Iterator<Item = (String, String)>,
    ) -> feignhttp::Result<Value> {
        // Todo, check for duplicates etc
        let mut variables = self.get_variables().await?;
        variables.variable_registry.variables = vars
            .map(|(name, value)| VariableDTO {
                variable: Variable { value, name },
            })
            .collect();
        self.update_variable_request(variables).await
    }

    pub async fn upload_template<S: Into<String>>(&self, content: S) -> feignhttp::Result<String> {
        let template = self.api_upload_template(content.into()).await?;
        let rot = simple_xml::from_string(&template).unwrap();

        let template = &rot["template"][0];

        Ok(template["id"][0].content.clone())
    }

    pub async fn instantiate_template(&self, id: &str) -> feignhttp::Result<FlowEntity> {
        let template_instance = serde_json::json!({
            "templateId": id,
            "originX":-1525.6531164510297,
            "originY":-204.79558937656458,
        });

        self.api_instantiate_template(template_instance).await
    }

    pub async fn instantiate_template_file<S: Into<String>>(
        &self,
        content: S,
    ) -> feignhttp::Result<FlowEntity> {
        let id = self.upload_template(content).await?;
        let created = self.instantiate_template(&id).await?;
        self.delete_template(&id).await?;

        Ok(created)
    }
}
