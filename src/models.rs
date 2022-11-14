use std::collections::HashMap;
use std::io::Write;

use serde::Deserialize;
use serde::Serialize;

use crate::rdf::prefix::CONN;
use crate::rdf::prefix::NIFI;
use crate::rdf::prefix::RDFS;
use crate::rdf::prefix::SH;
use crate::rdf::prefix::XSD;
use crate::rdf::RdfContext;
use crate::rdf::ToRDF;

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

impl ToRDF for &DocumentedTypeDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&RDFS);
        ctx.add_prefix(&CONN);
    }

    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        write!(buf, "[] a :{}; ", self.ty)?;
        if let Some(ref desc) = self.description {
            write!(buf, "   rdfs:description {:?};", desc)?;
        }

        self.tags
            .iter()
            .try_for_each(|x| write!(buf, " :tag {:?};", x))?;

        write!(buf, ".")?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerServiceApiDTO {
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerServiceDTO {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: Option<String>,
    pub descriptors: HashMap<String, DescriptorDTO>,
}

impl ToRDF for &ControllerServiceDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&CONN);
        ctx.add_prefix(&SH);
        ctx.add_prefix(&NIFI);
        <&DescriptorDTO>::add_ctx(ctx);
    }

    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        write!(
            buf,
            r#"
    <{}> a <NifiProcess>;
      :processProperties [
        nifi:type {:?};
      ];
      :shape [
        a sh:NodeShape;
        sh:targetClass <{}>; "#,
            self.name, self.ty, self.name
        )?;

        self.descriptors.values().try_for_each(|x| x.to_rdf(buf))?;

        write!(buf, "].",)?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionedEntity<Comp> {
    pub id: String,
    pub revision: RevisionDTO,
    pub component: Comp,
}

impl<T> ToRDF for &VersionedEntity<T>
where
    for<'a> &'a T: ToRDF,
{
    fn add_ctx(ctx: &mut RdfContext) {
        <&T>::add_ctx(ctx);
    }
    fn to_rdf(self, target: &mut impl Write) -> std::io::Result<()> {
        self.component.to_rdf(target)
    }
}

pub type ProcessorEntity = VersionedEntity<ProcessorDTO>;
pub type ControllerServiceEntity = VersionedEntity<ControllerServiceDTO>;

#[derive(Debug, Serialize, Deserialize)]
pub struct RevisionDTO {
    pub version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorDTO {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: Option<String>,

    pub relationships: Vec<RelationshipDTO>,
    pub config: ProcessorConfigDTO,
}

impl ToRDF for &ProcessorDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&NIFI);

        ctx.add_prefix(&CONN);
        ctx.add_prefix(&SH);
        <&RelationshipDTO>::add_ctx(ctx);
        <&DescriptorDTO>::add_ctx(ctx);
    }
    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        write!(
            buf,
            r#"
    <{}> a <NifiProcess>;
      :processProperties [
        nifi:type {:?};
      ];
      :shape [
        a sh:NodeShape;
        sh:targetClass <{}>;
        sh:property [
          sh:class :ReaderChannel;
          sh:path <INCOMING_CHANNEL>;
          sh:name "Incoming channel";
          sh:description "Combination of all incoming channels";
        ];
        "#,
            self.name, self.ty, self.name
        )?;

        self.relationships.iter().try_for_each(|x| x.to_rdf(buf))?;
        self.config
            .descriptors
            .values()
            .try_for_each(|x| x.to_rdf(buf))?;

        write!(buf, "] .")?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelationshipDTO {
    pub name: String,
    pub description: Option<String>,
}

impl ToRDF for &RelationshipDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&SH);
        ctx.add_prefix(&NIFI);
    }
    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        let path = make_path_safe(&self.name);

        write!(
            buf,
            r#" sh:property [
          sh:class :WriterChannel;
          sh:path <{}>;
          nifi:key {:?};
          sh:name {:?};   "#,
            path, self.name, self.name
        )?;

        if let Some(ref desc) = self.description {
            write!(buf, "  sh:description {:?};", desc)?;
        }
        write!(buf, "];")?;

        Ok(())
    }
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
fn make_path_safe(path: &str) -> String {
    path.chars()
        .map(|x| {
            x.is_alphabetic()
                .then_some(x)
                .map(|y| y.to_ascii_lowercase())
                .unwrap_or('-')
        })
        .collect()
}

impl ToRDF for &DescriptorDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&SH);
        ctx.add_prefix(&XSD);
    }
    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        let path = make_path_safe(&self.name);
        write!(
            buf,
            r#"
    sh:property [
          sh:datatype xsd:string;
          sh:path <{}>;
          sh:name {:?};
          sh:description {:?};
          sh:minCount {};
          nifi:key {:?}; "#,
            path, self.display, self.description, self.required as u32, self.name
        )?;

        if let Some(ref df) = self.default_value {
            write!(buf, "sh:defaultValue {:?};", df)?;
        }

        write!(buf, "] ;")?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionEntity {
    id: Option<String>,
    revision: RevisionDTO,
    component: ConnectionDTO,
    source_id: String,
    source_type: ConnectionTargetType,
    destination_id: String,
    destination_type: ConnectionTargetType,
}
impl ConnectionEntity {
    pub fn new(source_id: &str, target_id: &str, rel: &str) -> Self {
        let rels = vec![rel.to_string()];
        let source = ConnectableDTO {
            id: source_id.to_string(),
            ty: ConnectionTargetType::Processor,
            group_id: "root".to_string(),
        };
        let destination = ConnectableDTO {
            id: target_id.to_string(),
            ty: ConnectionTargetType::Processor,
            group_id: "root".to_string(),
        };
        let component = ConnectionDTO {
            source,
            destination,
            selected_relationships: rels.clone(),
            available_relationships: rels,
        };

        Self {
            id: None,
            revision: RevisionDTO { version: 0 },
            component,
            source_id: source_id.to_string(),
            source_type: ConnectionTargetType::Processor,
            destination_id: target_id.to_string(),
            destination_type: ConnectionTargetType::Processor,
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
