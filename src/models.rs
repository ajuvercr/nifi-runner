use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerServiceTypesEntity {
    #[serde(rename = "controllerServiceTypes")]
    pub types: Vec<DocumentedTypeDTO>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessTypesEntity {
    #[serde(rename = "processorTypes")]
    pub types: Vec<DocumentedTypeDTO>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentedTypeDTO {
    #[serde(rename = "type")]
    pub ty: String,
    pub description: Option<String>,
    #[serde(rename = "controllerServiceApis")]
    pub apis: Option<Vec<ControllerServiceApiDTO>>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerServiceApiDTO {
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerServiceDTO {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: Option<String>,
    pub descriptors: HashMap<String, DescriptorDTO>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionedEntity<Comp> {
    pub id: String,
    pub revision: RevisionDTO,
    pub component: Component<Comp>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Component<T> {
    pub id: String,
    #[serde(rename = "parentGroupId")]
    pub parent_group_id: String,
    #[serde(flatten)]
    pub comp: T,
}

pub type ProcessGroupEntity = VersionedEntity<ProcessGroupDTO>;
pub type ProcessorEntity = VersionedEntity<ProcessorDTO>;
pub type ControllerServiceEntity = VersionedEntity<ControllerServiceDTO>;
pub type PortEntity = VersionedEntity<PortDTO>;

#[derive(Debug, Serialize, Deserialize)]
pub struct RevisionDTO {
    #[serde(rename = "clientId")]
    pub client_id: Option<String>,
    pub version: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortDTO {
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorDTO {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: Option<String>,

    pub relationships: Vec<RelationshipDTO>,
    pub config: ProcessorConfigDTO,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelationshipDTO {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorConfigDTO {
    pub properties: HashMap<String, Option<String>>,
    pub descriptors: HashMap<String, DescriptorDTO>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DescriptorDTO {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display: String,
    pub description: String,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<String>,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionEntity {
    id: Option<String>,
    revision: RevisionDTO,
    component: ConnectionDTO,
}

impl ConnectionEntity {
    pub fn new<I, O>(input: &Component<I>, output: &Component<O>, rel: Option<&str>) -> Self
    where
        for<'a> &'a I: Into<ConnectionTargetType>,
        for<'a> &'a O: Into<ConnectionTargetType>,
    {
        let rels = rel.map(|x| vec![x.to_string()]).unwrap_or_default();
        let source = ConnectableDTO {
            id: input.id.to_string(),
            ty: (&input.comp).into(),
            group_id: input.parent_group_id.to_string(),
        };
        let destination = ConnectableDTO {
            id: output.id.to_string(),
            ty: (&output.comp).into(),
            group_id: output.parent_group_id.to_string(),
        };
        let component = ConnectionDTO {
            source,
            destination,
            selected_relationships: rels.clone(),
            available_relationships: rels,
        };

        Self {
            id: None,
            revision: RevisionDTO {
                client_id: None,
                version: 0,
            },
            component,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectionTargetType {
    Processor,
    RemoteInputPort,
    RemoteOutputPort,
    InputPort,
    OutputPort,
    Funnel,
}

impl<'a> From<&'a PortDTO> for ConnectionTargetType {
    fn from(this: &'a PortDTO) -> Self {
        match this.ty.as_str() {
            "INPUT_PORT" => ConnectionTargetType::InputPort,
            "OUTPUT_PORT" => ConnectionTargetType::OutputPort,
            _ => panic!(),
        }
    }
}

impl<'a> From<&'a ProcessorDTO> for ConnectionTargetType {
    fn from(_: &'a ProcessorDTO) -> Self {
        Self::Processor
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDTO {
    source: ConnectableDTO,
    destination: ConnectableDTO,
    selected_relationships: Vec<String>,
    available_relationships: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectableDTO {
    id: String,
    #[serde(rename = "type")]
    ty: ConnectionTargetType,
    group_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessGroupDTO {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortsEntity {
    #[serde(alias = "inputPorts")]
    #[serde(alias = "outputPorts")]
    pub ports: Vec<PortEntity>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub process_groups: Vec<ProcessGroupEntity>,
    pub processors: Vec<ProcessorEntity>,
    pub input_ports: Vec<PortEntity>,
    pub output_ports: Vec<PortEntity>,
    pub connections: Vec<ConnectionEntity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowEntity {
    pub flow: Flow,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariableRegistryEntity {
    #[serde(rename = "processGroupRevision")]
    pub revision: RevisionDTO,
    #[serde(rename = "variableRegistry")]
    pub variable_registry: VariableRegistry,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariableRegistry {
    #[serde(rename = "processGroupId")]
    pub group: String,
    pub variables: Vec<VariableDTO>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariableDTO {
    pub variable: Variable,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
}
