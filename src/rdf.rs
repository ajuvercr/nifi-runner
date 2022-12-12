use std::{collections::HashSet, io::Write};

use crate::models::{
    ControllerServiceDTO, DescriptorDTO, DocumentedTypeDTO, ProcessorDTO, RelationshipDTO,
    VersionedEntity,
};

use self::prefix::Prefix;

pub mod prefix {
    pub type Prefix = (&'static str, &'static str);
    pub static SH: Prefix = ("sh", "http://www.w3.org/ns/shacl#");
    pub static XSD: Prefix = ("xsd", "http://www.w3.org/2001/XMLSchema#");
    pub static DCTERMS: Prefix = ("dcterms", "http://purl.org/dc/terms/");
    pub static RDFS: Prefix = ("rdfs", "http://www.w3.org/2000/01/rdf-schema#");
    pub static SDS: Prefix = ("sds", "https://w3id.org/sds#");
    pub static CONN: Prefix = ("", "https://w3id.org/conn#");
    pub static NIFI: Prefix = ("nifi", "https://w3id.org/conn/nifi#");
    pub static FNO: Prefix = ("fno", "https://w3id.org/function/ontology#");
    pub static FNOM: Prefix = ("fnom", "https://w3id.org/function/vocabulary/mapping#");
}
use prefix::*;

#[derive(Default, Debug)]
pub struct RdfContext {
    prefixes: HashSet<Prefix>,
}

fn prefix_to_string(prefix: &Prefix) -> String {
    format!("@prefix {}: <{}> .\n", prefix.0, prefix.1)
}

impl RdfContext {
    pub fn add_prefix(&mut self, prefix: &Prefix) {
        self.prefixes.insert(prefix.clone());
    }

    pub fn prefixes(&self) -> String {
        self.prefixes.iter().map(prefix_to_string).collect()
    }
}

pub trait ToRDF {
    fn add_ctx(ctx: &mut RdfContext);
    fn to_rdf(self, target: &mut impl Write) -> std::io::Result<()>;
}

impl<T> ToRDF for &Vec<T>
where
    for<'a> &'a T: ToRDF,
{
    fn add_ctx(ctx: &mut RdfContext) {
        <&T>::add_ctx(ctx);
    }

    fn to_rdf(self, target: &mut impl Write) -> std::io::Result<()> {
        self.into_iter().try_for_each(|x| {
            x.to_rdf(target)?;
            write!(target, "\n")?;
            Ok(())
        })
    }
}

impl ToRDF for &DocumentedTypeDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&RDFS);
        ctx.add_prefix(&CONN);
    }

    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        write!(buf, "[] a :{};\n", self.ty)?;
        if let Some(ref desc) = self.description {
            write!(buf, "  rdfs:description {:?};\n", desc)?;
        }

        self.tags
            .iter()
            .try_for_each(|x| write!(buf, "  :tag {:?};\n", x))?;

        write!(buf, ".")?;

        Ok(())
    }
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
    nifi:{} a nifi:NifiProcess;
        nifi:type {:?}.

    [] a sh:NodeShape;
        sh:targetClass nifi:{}; "#,
            self.name, self.ty, self.name
        )?;

        self.descriptors.values().try_for_each(|x| x.to_rdf(buf))?;

        write!(buf, ".",)?;

        Ok(())
    }
}
impl<T, S> ToRDF for &VersionedEntity<T, S>
where
    for<'a> &'a T: ToRDF,
{
    fn add_ctx(ctx: &mut RdfContext) {
        <&T>::add_ctx(ctx);
    }
    fn to_rdf(self, target: &mut impl Write) -> std::io::Result<()> {
        self.component.comp.to_rdf(target)
    }
}

impl ToRDF for &ProcessorDTO {
    fn add_ctx(ctx: &mut RdfContext) {
        ctx.add_prefix(&NIFI);

        ctx.add_prefix(&CONN);
        ctx.add_prefix(&SH);
        ctx.add_prefix(&FNO);
        ctx.add_prefix(&FNOM);
        <&RelationshipDTO>::add_ctx(ctx);
        <&DescriptorDTO>::add_ctx(ctx);
    }
    fn to_rdf(self, buf: &mut impl Write) -> std::io::Result<()> {
        let desc_mapping: String = self
            .config
            .descriptors
            .iter()
            .map(|(_, x)| {
                let path = make_path_safe(&x.name);
                format!(
                    "nifi:mapping [ fno:parameterMapping [ fnom:functionParameter nifi:{}; fnom:implementationParameterPosition {:?} ] ]; \n",
                    path, x.name
                )
            }).collect();

        let rel_mapping: String =  self
            .relationships
            .iter()
            .map(|x| {
                let path = make_path_safe(&x.name);
                format!(
                    "nifi:mapping [ fno:parameterMapping [ fnom:functionParameter nifi:{}; fnom:implementationParameterPosition {:?} ] ]; \n",
                    path, x.name
                )
            }).collect();

        write!(
            buf,
            r#"
    nifi:{} a nifi:NifiProcess;
        {}{}
        nifi:type {:?}.

    [] a sh:NodeShape;
       sh:targetClass nifi:{};
       sh:property [
          sh:class :ReaderChannel;
          sh:path nifi:INCOMING_CHANNEL;
          sh:name "Incoming channel";
          sh:description "Combination of all incoming channels";
        ];
        "#,
            self.name, desc_mapping, rel_mapping, self.ty, self.name
        )?;

        self.relationships.iter().try_for_each(|x| x.to_rdf(buf))?;
        self.config
            .descriptors
            .values()
            .try_for_each(|x| x.to_rdf(buf))?;

        write!(buf, ".")?;
        Ok(())
    }
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
          sh:path nifi:{};
          sh:name {:?};   "#,
            path, self.name
        )?;

        if let Some(ref desc) = self.description {
            write!(buf, "  sh:description {:?};", desc)?;
        }
        write!(buf, "];")?;

        Ok(())
    }
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
          sh:path nifi:{};
          sh:name {:?};
          sh:description {:?};
          sh:minCount {};"#,
            path, self.display, self.description, self.required as u32
        )?;

        if let Some(ref df) = self.default_value {
            write!(buf, "sh:defaultValue {:?};", df)?;
        }

        write!(buf, "] ;")?;
        Ok(())
    }
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
